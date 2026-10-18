//! GIF / WebP 动图播放:后台线程流式解码,UI 按每帧延迟播放。
//!
//! 帧通过容量很小的有界通道传递(播放暂停时线程自然阻塞,内存有界);
//! 每帧在解码后即缩放到显示尺寸,避免大动图撑爆内存。

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::{bounded, Receiver, Sender};
use easyshow_core::ImageBuf;

pub enum AnimEvent {
    /// 动画总帧数(单次循环)
    Info { frames: u32 },
    Frame { image: Arc<ImageBuf>, delay_ms: u32 },
    Failed { message: String },
    Done,
}

pub struct AnimPlayer {
    rx: Receiver<AnimEvent>,
    stop: Arc<AtomicBool>,
}

impl AnimPlayer {
    pub fn start(path: &Path, target: Option<(u32, u32)>) -> Option<Self> {
        let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
        if ext != "gif" && ext != "webp" {
            return None;
        }
        let (tx, rx) = bounded::<AnimEvent>(4);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_worker = stop.clone();
        let path_owned = path.to_path_buf();
        std::thread::Builder::new()
            .name("easyshow-anim".into())
            .spawn(move || {
                if let Err(message) = run(&path_owned, target, &tx, &stop_worker) {
                    let _ = tx.send(AnimEvent::Failed { message });
                }
                let _ = tx.send(AnimEvent::Done);
            })
            .ok()?;
        Some(Self { rx, stop })
    }

    pub fn try_recv(&self) -> Option<AnimEvent> {
        self.rx.try_recv().ok()
    }
}

impl Drop for AnimPlayer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn run(
    path: &Path,
    target: Option<(u32, u32)>,
    tx: &Sender<AnimEvent>,
    stop: &AtomicBool,
) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取动图失败: {e}"))?;
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "gif" => run_gif(&bytes, target, tx, stop),
        "webp" => run_webp(&bytes, target, tx, stop),
        _ => Ok(()),
    }
}

fn downscale(image: ImageBuf, target: Option<(u32, u32)>) -> ImageBuf {
    let Some((max_w, max_h)) = target else { return image };
    let scale = easyshow_core::resize::fit_scale(
        image.width,
        image.height,
        max_w.max(1),
        max_h.max(1),
    );
    if scale >= 0.98 {
        return image;
    }
    let nw = ((image.width as f64 * scale).round() as u32).max(1);
    let nh = ((image.height as f64 * scale).round() as u32).max(1);
    easyshow_core::resize::resize_rgba(&image, nw, nh).unwrap_or(image)
}

fn send_frame(tx: &Sender<AnimEvent>, image: ImageBuf, delay_ms: u32) -> bool {
    tx.send(AnimEvent::Frame { image: Arc::new(image), delay_ms: delay_ms.max(20) }).is_ok()
}

fn run_gif(
    bytes: &[u8],
    target: Option<(u32, u32)>,
    tx: &Sender<AnimEvent>,
    stop: &AtomicBool,
) -> Result<(), String> {
    use image::AnimationDecoder;

    let probe = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("gif: {e}"))?;
    let total_loops = match probe.loop_count() {
        image::metadata::LoopCount::Infinite => u32::MAX,
        image::metadata::LoopCount::Finite(n) => n.get().max(1),
    };

    let mut played = 0u32;
    let mut announced = false;
    while played < total_loops {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes))
            .map_err(|e| format!("gif: {e}"))?;
        let mut emitted = 0u32;
        for frame in decoder.into_frames() {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let frame = frame.map_err(|e| format!("gif 帧: {e}"))?;
            let (num, den) = frame.delay().numer_denom_ms();
            let delay_ms = if den == 0 { 100 } else { (num as f64 / den as f64).round() as u32 };
            let buffer = frame.into_buffer();
            let image = ImageBuf::new(buffer.width(), buffer.height(), buffer.into_raw());
            if !send_frame(tx, downscale(image, target), delay_ms) {
                return Ok(());
            }
            emitted += 1;
        }
        if emitted == 0 {
            return Ok(());
        }
        if !announced {
            announced = true;
            let _ = tx.send(AnimEvent::Info { frames: emitted });
        }
        if emitted <= 1 {
            return Ok(()); // 静态 GIF,无需循环
        }
        played += 1;
    }
    Ok(())
}

fn run_webp(
    bytes: &[u8],
    target: Option<(u32, u32)>,
    tx: &Sender<AnimEvent>,
    stop: &AtomicBool,
) -> Result<(), String> {
    use image_webp::{LoopCount, WebPDecoder};

    let make = || WebPDecoder::new(std::io::Cursor::new(bytes));
    let mut decoder = make().map_err(|e| format!("webp: {e}"))?;
    if !decoder.is_animated() {
        return Ok(());
    }
    let frames = decoder.num_frames();
    if frames <= 1 {
        return Ok(());
    }
    let (w, h) = decoder.dimensions();
    let has_alpha = decoder.has_alpha();
    let buffer_len = decoder
        .output_buffer_size()
        .ok_or_else(|| "webp: 尺寸过大".to_string())?;
    let total_loops = match decoder.loop_count() {
        LoopCount::Forever => u32::MAX,
        LoopCount::Times(n) => n.get() as u32,
    };
    let _ = tx.send(AnimEvent::Info { frames });

    let mut buf = vec![0u8; buffer_len];
    let mut played = 0u32;
    while played < total_loops {
        for i in 0..frames {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let delay_ms = decoder
                .read_frame(&mut buf)
                .map_err(|e| format!("webp 帧 {i}: {e}"))?;
            let image = if has_alpha {
                ImageBuf::new(w, h, buf.clone())
            } else {
                let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
                for px in buf.chunks_exact(3) {
                    rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
                ImageBuf::new(w, h, rgba)
            };
            if !send_frame(tx, downscale(image, target), delay_ms) {
                return Ok(());
            }
        }
        played += 1;
        if played < total_loops {
            decoder = make().map_err(|e| format!("webp: {e}"))?;
        }
    }
    Ok(())
}
