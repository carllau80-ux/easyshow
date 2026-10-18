//! 目录级 EXIF 拍摄时间扫描:后台读取 DateTimeOriginal,供时间排序与
//! 年/月/日时间轴使用。
//!
//! 与 GPS 扫描同一套模式:单工作线程、generation 取消、批量回传,
//! 结果按 path+mtime 缓存到 `<cache>/times/<dir-hash>.json`。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crossbeam_channel::{unbounded, Receiver, Sender};
use easyshow_core::DateTimeParts;

use crate::thumbs::cache_root;

pub struct ScanItem {
    pub index: usize,
    pub path: PathBuf,
    pub mtime: u64,
}

pub struct ScanEvent {
    pub generation: u64,
    pub results: Vec<(usize, PathBuf, Option<DateTimeParts>)>,
    pub done: bool,
}

pub struct TimeScanService {
    tx: Sender<(u64, PathBuf, Vec<ScanItem>)>,
    rx: Receiver<ScanEvent>,
    latest: Arc<AtomicU64>,
}

impl TimeScanService {
    pub fn new() -> Self {
        let (tx, req_rx) = unbounded::<(u64, PathBuf, Vec<ScanItem>)>();
        let (reply_tx, rx) = unbounded::<ScanEvent>();
        let latest = Arc::new(AtomicU64::new(0));
        let latest_worker = latest.clone();

        let spawned = std::thread::Builder::new()
            .name("easyshow-timescan".into())
            .spawn(move || {
                for (generation, dir, items) in req_rx {
                    if generation < latest_worker.load(Ordering::Relaxed) {
                        continue;
                    }
                    let mut index = load_index(&dir);
                    let mut batch: Vec<(usize, PathBuf, Option<DateTimeParts>)> = Vec::with_capacity(16);
                    let mut changed = false;

                    for item in items {
                        if generation < latest_worker.load(Ordering::Relaxed) {
                            break;
                        }
                        let key = item.path.to_string_lossy().to_string();
                        let taken = match index.get(&key) {
                            Some(&(mtime, parts)) if mtime == item.mtime => parts.map(parts_from_array),
                            _ => {
                                let taken = easyshow_core::read_taken_time(&item.path);
                                index.insert(key, (item.mtime, taken.map(parts_to_array)));
                                changed = true;
                                taken
                            }
                        };
                        batch.push((item.index, item.path, taken));
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
            log::warn!("timescan: 工作线程创建失败: {e}");
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

fn parts_to_array(p: DateTimeParts) -> [i32; 6] {
    [p.year, p.month as i32, p.day as i32, p.hour as i32, p.minute as i32, p.second as i32]
}

fn parts_from_array(a: [i32; 6]) -> DateTimeParts {
    DateTimeParts {
        year: a[0],
        month: a[1].clamp(1, 12) as u8,
        day: a[2].clamp(1, 31) as u8,
        hour: a[3].clamp(0, 23) as u8,
        minute: a[4].clamp(0, 59) as u8,
        second: a[5].clamp(0, 59) as u8,
    }
}

/// path → (mtime, 拍摄时间或 None)
type Index = HashMap<String, (u64, Option<[i32; 6]>)>;

fn index_path(dir: &Path) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    dir.to_string_lossy().hash(&mut hasher);
    Some(cache_root()?.join("times").join(format!("{:016x}.json", hasher.finish())))
}

fn load_index(dir: &Path) -> Index {
    let mut out = Index::new();
    let Some(path) = index_path(dir) else { return out };
    let Ok(text) = std::fs::read_to_string(path) else { return out };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return out };
    let Some(entries) = v.get("entries").and_then(|e| e.as_object()) else { return out };
    for (key, val) in entries {
        let Some(mtime) = val.get("m").and_then(|x| x.as_u64()) else { continue };
        let taken = val.get("t").and_then(|x| x.as_array()).and_then(|arr| {
            let mut parts = [0i32; 6];
            for (i, v) in arr.iter().take(6).enumerate() {
                parts[i] = v.as_i64()? as i32;
            }
            Some(parts)
        });
        out.insert(key.clone(), (mtime, taken));
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
    for (key, (mtime, taken)) in index {
        entries.insert(
            key.clone(),
            serde_json::json!({ "m": mtime, "t": taken }),
        );
    }
    let body = serde_json::json!({ "v": 2, "entries": entries });
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_vec(&body).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}
