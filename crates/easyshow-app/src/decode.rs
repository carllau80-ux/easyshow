//! Background decoding service: a small worker pool with generation-based
//! cancellation, so switching images quickly never blocks on stale work.
//! Requests carry a purpose (current image vs. prefetch) so the UI can treat
//! the results differently.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crossbeam_channel::{unbounded, Receiver, Sender};

use easyshow_core::{decode_file_scaled, ImageBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Decode for immediate display.
    Current,
    /// Decode ahead of time (cached in memory for instant next/prev).
    Prefetch,
}

#[derive(Debug)]
struct DecodeRequest {
    path: PathBuf,
    generation: u64,
    /// Optional display-size cap; `None` requests full resolution.
    max_dim: Option<(u32, u32)>,
    purpose: Purpose,
}

pub struct Decoded {
    pub path: PathBuf,
    pub generation: u64,
    pub image: Arc<ImageBuf>,
    pub downscaled: bool,
    pub purpose: Purpose,
}

pub enum DecodeEvent {
    Done(Decoded),
    Failed {
        path: PathBuf,
        generation: u64,
        message: String,
        purpose: Purpose,
    },
}

pub struct DecodeService {
    tx: Sender<DecodeRequest>,
    rx: Receiver<DecodeEvent>,
    latest: Arc<AtomicU64>,
}

impl DecodeService {
    pub fn new(workers: usize) -> Self {
        let (req_tx, req_rx) = unbounded::<DecodeRequest>();
        let (ev_tx, ev_rx) = unbounded::<DecodeEvent>();
        let latest = Arc::new(AtomicU64::new(0));

        for i in 0..workers.max(1) {
            let req_rx = req_rx.clone();
            let ev_tx = ev_tx.clone();
            let latest = latest.clone();
            std::thread::Builder::new()
                .name(format!("easyshow-decode-{i}"))
                .spawn(move || {
                    while let Ok(req) = req_rx.recv() {
                        // Superseded while queued — nothing to do.
                        if req.generation < latest.load(Ordering::SeqCst) {
                            continue;
                        }
                        let result = decode_one(&req);
                        // Superseded while decoding — drop quietly.
                        if req.generation < latest.load(Ordering::SeqCst) {
                            continue;
                        }
                        let event = match result {
                            Ok((image, downscaled)) => DecodeEvent::Done(Decoded {
                                path: req.path,
                                generation: req.generation,
                                image: Arc::new(image),
                                downscaled,
                                purpose: req.purpose,
                            }),
                            Err(err) => DecodeEvent::Failed {
                                path: req.path,
                                generation: req.generation,
                                message: err.to_string(),
                                purpose: req.purpose,
                            },
                        };
                        if ev_tx.send(event).is_err() {
                            break; // UI is gone
                        }
                    }
                })
                .expect("spawn decode worker");
        }

        Self { tx: req_tx, rx: ev_rx, latest }
    }

    /// Queue a decode request; anything older becomes obsolete immediately.
    pub fn request(&self, path: PathBuf, generation: u64, max_dim: Option<(u32, u32)>, purpose: Purpose) {
        self.latest.store(generation, Ordering::SeqCst);
        let _ = self.tx.send(DecodeRequest { path, generation, max_dim, purpose });
    }

    pub fn try_recv(&self) -> Option<DecodeEvent> {
        self.rx.try_recv().ok()
    }
}

fn decode_one(req: &DecodeRequest) -> easyshow_core::Result<(ImageBuf, bool)> {
    // 把目标尺寸传给 core:SVG 等矢量格式会直接按显示尺寸渲染。
    let mut image = decode_file_scaled(&req.path, req.max_dim)?;
    if let Some(orientation) = easyshow_core::exif::orientation_from_file(&req.path) {
        image = easyshow_core::exif::apply_orientation(image, orientation);
    }
    let mut downscaled = false;
    if let Some((max_w, max_h)) = req.max_dim {
        let scale = easyshow_core::resize::fit_scale(image.width, image.height, max_w, max_h);
        if scale < 0.98 {
            let new_w = ((image.width as f64 * scale).round() as u32).max(1);
            let new_h = ((image.height as f64 * scale).round() as u32).max(1);
            image = easyshow_core::resize::resize_rgba(&image, new_w, new_h)?;
            downscaled = true;
        }
    }
    Ok((image, downscaled))
}
