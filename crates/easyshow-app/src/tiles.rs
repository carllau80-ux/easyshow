//! 在线地图瓦片:按视口下载、磁盘缓存、后台解码。
//!
//! 与缩略图服务同一套模式:crossbeam 通道 + 工作线程 + generation + wanted 可见集。
//! 瓦片按 `<cache>/tiles/<source>/<z>/<x>/<y>.bin` 缓存原始响应(再次打开离线可用)。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender};
use easyshow_core::ImageBuf;

use crate::config::TileSource;
use crate::thumbs::cache_root;

/// 瓦片源(高德为 GCJ-02 坐标;OSM 为 WGS-84)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileKind {
    Amap,
    Osm,
}

impl TileKind {
    pub fn tag(self) -> &'static str {
        match self {
            TileKind::Amap => "amap",
            TileKind::Osm => "osm",
        }
    }

    /// 配置里的来源 → 实际瓦片源(Auto 不产生瓦片源,由调用方按语言解析)。
    pub fn from_source(source: TileSource) -> Option<Self> {
        match source {
            TileSource::Amap => Some(TileKind::Amap),
            TileSource::Osm => Some(TileKind::Osm),
            TileSource::Auto => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

pub struct TileRequest {
    pub generation: u64,
    pub kind: TileKind,
    pub key: TileKey,
}

pub struct TileReply {
    pub generation: u64,
    pub kind: TileKind,
    pub key: TileKey,
    pub image: Option<ImageBuf>,
}

pub struct TileService {
    tx: Sender<TileRequest>,
    rx: Receiver<TileReply>,
    latest: Arc<AtomicU64>,
    wanted: Arc<RwLock<HashSet<(TileKind, TileKey)>>>,
}

impl TileService {
    pub fn new() -> Self {
        let (tx, req_rx) = unbounded::<TileRequest>();
        let (reply_tx, rx) = unbounded::<TileReply>();
        let latest = Arc::new(AtomicU64::new(0));
        let wanted = Arc::new(RwLock::new(HashSet::new()));

        for worker in 0..2 {
            let req_rx = req_rx.clone();
            let reply_tx = reply_tx.clone();
            let latest = latest.clone();
            let wanted = wanted.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("easyshow-tiles-{worker}"))
                .spawn(move || {
                    let agent = ureq::Agent::config_builder()
                        .timeout_global(Some(Duration::from_secs(8)))
                        .user_agent(format!(
                            "EasyShow/{} (https://github.com/carllau80-ux/easyshow)",
                            env!("CARGO_PKG_VERSION")
                        ))
                        .build()
                        .into();
                    for req in &req_rx {
                        log::debug!("tiles: req {:?} {}/{}/{}", req.kind, req.key.z, req.key.x, req.key.y);
                        if req.generation < latest.load(Ordering::Relaxed) {
                            log::debug!("tiles: stale generation");
                            continue;
                        }
                        let visible = wanted
                            .read()
                            .map(|w| w.contains(&(req.kind, req.key)))
                            .unwrap_or(true);
                        if !visible {
                            log::debug!("tiles: not in visible set");
                            continue; // 已滚出视口
                        }
                        let image = load_tile(&agent, req.kind, req.key);
                        log::debug!("tiles: loaded={} {:?}", image.is_some(), req.key);
                        if req.generation < latest.load(Ordering::Relaxed) {
                            continue;
                        }
                        if reply_tx
                            .send(TileReply {
                                generation: req.generation,
                                kind: req.kind,
                                key: req.key,
                                image,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            if let Err(e) = spawned {
                log::warn!("tiles: 工作线程创建失败: {e}");
            }
        }

        Self { tx, rx, latest, wanted }
    }

    /// 更新可见瓦片集(视口变化时调用)。
    pub fn set_wanted(&self, kind: TileKind, keys: &HashSet<TileKey>) {
        if let Ok(mut wanted) = self.wanted.write() {
            let target: HashSet<(TileKind, TileKey)> =
                keys.iter().map(|&k| (kind, k)).collect();
            if *wanted != target {
                *wanted = target;
            }
        }
    }

    pub fn clear_wanted(&self) {
        if let Ok(mut wanted) = self.wanted.write() {
            wanted.clear();
        }
    }

    pub fn request(&self, kind: TileKind, key: TileKey, generation: u64) {
        let _ = self.tx.send(TileRequest { generation, kind, key });
    }

    /// 新的 generation(切换底图/开关联网时使旧请求失效)。
    pub fn bump_generation(&self) -> u64 {
        self.latest.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn try_recv(&self) -> Option<TileReply> {
        self.rx.try_recv().ok()
    }
}

fn tile_cache_path(kind: TileKind, key: TileKey) -> Option<PathBuf> {
    Some(
        cache_root()?
            .join("tiles")
            .join(kind.tag())
            .join(key.z.to_string())
            .join(key.x.to_string())
            .join(format!("{}.bin", key.y)),
    )
}

fn load_tile(agent: &ureq::Agent, kind: TileKind, key: TileKey) -> Option<ImageBuf> {
    let bytes = if let Some(path) = tile_cache_path(kind, key) {
        match std::fs::read(&path) {
            Ok(bytes) if !bytes.is_empty() => bytes,
            _ => fetch_and_cache(agent, kind, key, &path)?,
        }
    } else {
        fetch_bytes(agent, kind, key)?
    };
    decode_tile(&bytes)
}

fn fetch_and_cache(
    agent: &ureq::Agent,
    kind: TileKind,
    key: TileKey,
    path: &PathBuf,
) -> Option<Vec<u8>> {
    let bytes = fetch_bytes(agent, kind, key)?;
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_ok() {
            let tmp = path.with_extension("bin.tmp");
            if std::fs::write(&tmp, &bytes).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }
    Some(bytes)
}

fn fetch_bytes(agent: &ureq::Agent, kind: TileKind, key: TileKey) -> Option<Vec<u8>> {
    let url = match kind {
        TileKind::Amap => {
            let sub = 1 + ((key.x as u64 + key.y as u64) % 4);
            format!(
                "https://webrd0{sub}.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=8&x={}&y={}&z={}",
                key.x, key.y, key.z
            )
        }
        TileKind::Osm => format!("https://tile.openstreetmap.org/{}/{}/{}.png", key.z, key.x, key.y),
    };
    let mut request = agent.get(&url);
    if kind == TileKind::Amap {
        request = request.header("Referer", "https://www.amap.com/");
    }
    let mut response = match request.call() {
        Ok(r) => r,
        Err(e) => {
            log::warn!("tiles: {url} 请求失败: {e}");
            return None;
        }
    };
    match response.body_mut().read_to_vec() {
        Ok(bytes) if !bytes.is_empty() => Some(bytes),
        Ok(_) => {
            log::warn!("tiles: {url} 返回空内容");
            None
        }
        Err(e) => {
            log::warn!("tiles: {url} 读取失败: {e}");
            None
        }
    }
}

fn decode_tile(bytes: &[u8]) -> Option<ImageBuf> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(ImageBuf::new(w, h, img.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_kind_from_source() {
        assert_eq!(TileKind::from_source(TileSource::Amap), Some(TileKind::Amap));
        assert_eq!(TileKind::from_source(TileSource::Osm), Some(TileKind::Osm));
        assert_eq!(TileKind::from_source(TileSource::Auto), None);
    }

    #[test]
    fn cache_path_shape() {
        // 仅在能确定缓存根目录时检查路径结构
        if cache_root().is_none() {
            return;
        }
        let path = tile_cache_path(TileKind::Amap, TileKey { z: 5, x: 26, y: 13 }).unwrap();
        let s = path.to_string_lossy();
        assert!(s.contains("/tiles/amap/5/26/13.bin"), "{s}");
    }
}
