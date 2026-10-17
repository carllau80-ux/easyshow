//! 目录级 GPS 扫描:后台读取每张照片的 GPS,供地图模式使用。
//!
//! - 单工作线程顺序读取(EXIF 头读取很快,几千张在数秒内完成);
//! - 按 `<cache>/geopoints/<dir-hash>.json` 缓存 path+mtime→坐标,重复打开目录秒出;
//! - 结果分批回传,UI 线程不阻塞。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::thumbs::cache_root;

pub struct ScanItem {
    pub index: usize,
    pub path: PathBuf,
    pub mtime: u64,
}

pub struct ScanEvent {
    pub generation: u64,
    pub results: Vec<(usize, Option<(f64, f64)>)>,
    pub done: bool,
}

pub struct GeoScanService {
    tx: Sender<(u64, PathBuf, Vec<ScanItem>)>,
    rx: Receiver<ScanEvent>,
    latest: Arc<AtomicU64>,
}

impl GeoScanService {
    pub fn new() -> Self {
        let (tx, req_rx) = unbounded::<(u64, PathBuf, Vec<ScanItem>)>();
        let (reply_tx, rx) = unbounded::<ScanEvent>();
        let latest = Arc::new(AtomicU64::new(0));
        let latest_worker = latest.clone();

        let spawned = std::thread::Builder::new()
            .name("easyshow-geoscan".into())
            .spawn(move || {
                for (generation, dir, items) in req_rx {
                    if generation < latest_worker.load(Ordering::Relaxed) {
                        continue;
                    }
                    let mut index = load_index(&dir);
                    let mut batch: Vec<(usize, Option<(f64, f64)>)> = Vec::with_capacity(16);
                    let mut changed = false;

                    for item in items {
                        if generation < latest_worker.load(Ordering::Relaxed) {
                            break; // 已切换到别的目录
                        }
                        let key = item.path.to_string_lossy().to_string();
                        let gps = match index.get(&key) {
                            Some(&(mtime, lat, lon)) if mtime == item.mtime => Some((lat, lon)),
                            cached => {
                                // 未缓存或文件已变更:重新读取
                                if cached.is_some() {
                                    index.remove(&key);
                                    changed = true;
                                }
                                let gps = easyshow_core::read_gps(&item.path);
                                match gps {
                                    Some((lat, lon)) => {
                                        index.insert(key, (item.mtime, lat, lon));
                                        changed = true;
                                    }
                                    None => {}
                                }
                                gps
                            }
                        };
                        batch.push((item.index, gps));
                        if batch.len() >= 16 {
                            let results = std::mem::take(&mut batch);
                            if reply_tx
                                .send(ScanEvent { generation, results, done: false })
                                .is_err()
                            {
                                return;
                            }
                        }
                    }

                    if changed {
                        save_index(&dir, &index);
                    }
                    if reply_tx.send(ScanEvent { generation, results: batch, done: true }).is_err() {
                        break;
                    }
                }
            });
        if let Err(e) = spawned {
            log::warn!("geoscan: 工作线程创建失败: {e}");
        }

        Self { tx, rx, latest }
    }

    pub fn start(&self, dir: PathBuf, items: Vec<ScanItem>) -> u64 {
        let generation = self.latest.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.tx.send((generation, dir, items));
        generation
    }

    pub fn try_recv(&self) -> Option<ScanEvent> {
        self.rx.try_recv().ok()
    }
}

/// 索引:path → (mtime 秒, 纬度, 经度)
type Index = HashMap<String, (u64, f64, f64)>;

fn index_path(dir: &Path) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    dir.to_string_lossy().hash(&mut hasher);
    Some(cache_root()?.join("geopoints").join(format!("{:016x}.json", hasher.finish())))
}

fn load_index(dir: &Path) -> Index {
    let mut out = Index::new();
    let Some(path) = index_path(dir) else { return out };
    let Ok(text) = std::fs::read_to_string(path) else { return out };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return out };
    let Some(entries) = v.get("entries").and_then(|e| e.as_object()) else { return out };
    for (key, val) in entries {
        let (Some(mtime), Some(lat), Some(lon)) = (
            val.get("m").and_then(|x| x.as_u64()),
            val.get("lat").and_then(|x| x.as_f64()),
            val.get("lon").and_then(|x| x.as_f64()),
        ) else {
            continue;
        };
        // 合理范围校验(经纬度)
        if lat.is_finite() && lon.is_finite() && (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
            out.insert(key.clone(), (mtime, lat, lon));
        }
    }
    out
}

fn save_index(dir: &Path, index: &Index) {
    let Some(path) = index_path(dir) else { return };
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let mut entries = serde_json::Map::new();
    for (key, &(mtime, lat, lon)) in index {
        entries.insert(
            key.clone(),
            serde_json::json!({ "m": mtime, "lat": lat, "lon": lon }),
        );
    }
    let body = serde_json::json!({ "v": 1, "entries": entries });
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_vec(&body).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}
