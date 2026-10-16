//! Thumbnail service: a small worker pool with an on-disk JPEG cache, so a
//! directory shows its grid instantly on revisits.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crossbeam_channel::{unbounded, Receiver, Sender};
use liteshow_core::ImageBuf;

/// Long edge of generated thumbnails, in pixels.
pub const THUMB_EDGE: u32 = 160;
const JPEG_QUALITY: u8 = 80;

pub struct ThumbEvent {
    pub path: PathBuf,
    pub generation: u64,
    pub result: Result<Arc<ImageBuf>, String>,
}

pub struct ThumbService {
    tx: Sender<(PathBuf, u64)>,
    rx: Receiver<ThumbEvent>,
    latest: Arc<AtomicU64>,
}

impl ThumbService {
    pub fn new(workers: usize) -> Self {
        let (tx, req_rx) = unbounded::<(PathBuf, u64)>();
        let (ev_tx, rx) = unbounded::<ThumbEvent>();
        let latest = Arc::new(AtomicU64::new(0));

        for i in 0..workers.max(1) {
            let req_rx = req_rx.clone();
            let ev_tx = ev_tx.clone();
            let latest = latest.clone();
            std::thread::Builder::new()
                .name(format!("liteshow-thumb-{i}"))
                .spawn(move || {
                    while let Ok((path, generation)) = req_rx.recv() {
                        if generation < latest.load(Ordering::SeqCst) {
                            continue; // 目录已切换
                        }
                        let result = thumb_one(&path).map(Arc::new);
                        if generation < latest.load(Ordering::SeqCst) {
                            continue;
                        }
                        if ev_tx.send(ThumbEvent { path, generation, result }).is_err() {
                            break;
                        }
                    }
                })
                .expect("spawn thumb worker");
        }

        Self { tx, rx, latest }
    }

    /// Queue a thumbnail request for the current directory generation.
    pub fn request(&self, path: PathBuf, generation: u64) {
        self.latest.store(generation, Ordering::SeqCst);
        let _ = self.tx.send((path, generation));
    }

    pub fn try_recv(&self) -> Option<ThumbEvent> {
        self.rx.try_recv().ok()
    }
}

/// Cache hit → return; miss → generate, store, return.
fn thumb_one(path: &Path) -> Result<ImageBuf, String> {
    let cache_file = cache_file_for(path);
    if let Some(file) = &cache_file {
        if let Ok(bytes) = std::fs::read(file) {
            if let Ok(image) = liteshow_core::thumb::decode_jpeg_bytes(&bytes) {
                return Ok(image);
            }
        }
    }

    let thumb = liteshow_core::thumb::make_thumbnail(path, THUMB_EDGE).map_err(|e| e.to_string())?;

    if let Some(file) = &cache_file {
        if let Ok(bytes) = liteshow_core::thumb::encode_jpeg(&thumb, JPEG_QUALITY) {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = file.with_extension("tmp");
            if std::fs::write(&tmp, &bytes).is_ok() {
                let _ = std::fs::rename(&tmp, file);
            }
        }
    }
    Ok(thumb)
}

/// `<cache>/liteshow/thumbs/<2-hex>/<hash>-<mtime>-<size>.jpg`
fn cache_file_for(path: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    let key = format!("{:016x}-{}-{}", hasher.finish(), mtime, meta.len());
    Some(cache_root()?.join("thumbs").join(&key[..2]).join(format!("{key}.jpg")))
}

fn cache_root() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Liteshow"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Caches/liteshow"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .map(|p| p.join("liteshow"))
    }
}
