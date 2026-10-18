//! 子目录卡片:扫描当前目录的直接子目录,找出其中含照片的文件夹。
//!
//! - 每个子目录递归最多 [`MAX_DEPTH`] 层统计照片数(达到 [`MAX_COUNT`] 即封顶);
//! - 封面从该子目录的照片中按路径哈希稳定地"随机"挑一张(重复打开不变,便于缓存);
//! - 单工作线程,结果分批回传,大目录下网格也能逐批出现,不阻塞 UI。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crossbeam_channel::{unbounded, Receiver, Sender};

/// 子目录内部继续深入的层数(0 = 只统计子目录自身)。
const MAX_DEPTH: usize = 2;
/// 防御性上限:单个子目录最多浏览的目录项数。
const MAX_VISIT: usize = 4000;
/// 照片计数上限;达到后显示 `MAX_COUNT+` 之类由 UI 处理。
pub const MAX_COUNT: usize = 1000;
/// 候选封面数量上限。
const MAX_COVERS: usize = 64;

#[derive(Debug, Clone)]
pub struct FolderCard {
    pub dir: PathBuf,
    pub name: String,
    pub photo_count: usize,
    /// 稳定随机挑选的封面照片(若扫描到照片则必有)。
    pub cover: Option<PathBuf>,
    /// 子目录内照片的最新修改时间(Unix 秒),用于排序。
    pub latest_mtime: u64,
}

pub struct FolderEvent {
    pub generation: u64,
    pub cards: Vec<FolderCard>,
    pub done: bool,
}

pub struct FolderService {
    tx: Sender<(u64, PathBuf)>,
    rx: Receiver<FolderEvent>,
    latest: Arc<AtomicU64>,
}

impl FolderService {
    pub fn new() -> Self {
        let (tx, req_rx) = unbounded::<(u64, PathBuf)>();
        let (reply_tx, rx) = unbounded::<FolderEvent>();
        let latest = Arc::new(AtomicU64::new(0));
        let latest_worker = latest.clone();

        let spawned = std::thread::Builder::new()
            .name("easyshow-folders".into())
            .spawn(move || {
                for (generation, dir) in req_rx {
                    if generation < latest_worker.load(Ordering::Relaxed) {
                        continue;
                    }
                    scan_parent(&dir, generation, &latest_worker, &reply_tx);
                }
            });
        if let Err(e) = spawned {
            log::warn!("folders: 工作线程创建失败: {e}");
        }

        Self { tx, rx, latest }
    }

    pub fn start(&self, dir: PathBuf) -> u64 {
        let generation = self.latest.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.tx.send((generation, dir));
        generation
    }

    pub fn try_recv(&self) -> Option<FolderEvent> {
        self.rx.try_recv().ok()
    }
}

/// 扫描 `parent` 的直接子目录,分批回传含照片的文件夹卡片。
fn scan_parent(
    parent: &Path,
    generation: u64,
    latest: &AtomicU64,
    tx: &Sender<FolderEvent>,
) {
    let Ok(read) = std::fs::read_dir(parent) else {
        let _ = tx.send(FolderEvent { generation, cards: Vec::new(), done: true });
        return;
    };
    let mut batch: Vec<FolderCard> = Vec::new();
    let mut cards: Vec<FolderCard> = Vec::new();
    for item in read.flatten() {
        if generation < latest.load(Ordering::Relaxed) {
            return;
        }
        let Ok(file_type) = item.file_type() else { continue };
        if !file_type.is_dir() {
            continue;
        }
        let path = item.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.is_empty() || name.starts_with('.') || name.starts_with('$') {
            continue;
        }
        if let Some(card) = summarize(&path, name) {
            cards.push(card);
            if cards.len() % 8 == 0 {
                batch.append(&mut cards);
                if tx
                    .send(FolderEvent { generation, cards: std::mem::take(&mut batch), done: false })
                    .is_err()
                {
                    return;
                }
            }
        }
    }
    batch.append(&mut cards);
    let _ = tx.send(FolderEvent { generation, cards: batch, done: true });
}

/// 统计一个子目录(含其下 [`MAX_DEPTH`] 层)的照片数量并挑选封面。
/// 不含任何照片时返回 `None`。
fn summarize(dir: &Path, name: String) -> Option<FolderCard> {
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    let mut visited = 0usize;
    let mut count = 0usize;
    let mut latest_mtime = 0u64;
    let mut covers: Vec<PathBuf> = Vec::new();

    'walk: while let Some((current, depth)) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&current) else { continue };
        for item in read.flatten() {
            visited += 1;
            if visited > MAX_VISIT {
                break 'walk;
            }
            let Ok(file_type) = item.file_type() else { continue };
            let path = item.path();
            if file_type.is_dir() {
                let hidden = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with('.') || n.starts_with('$'))
                    .unwrap_or(true);
                if !hidden && depth < MAX_DEPTH {
                    stack.push((path, depth + 1));
                }
            } else if file_type.is_file() && easyshow_core::from_extension(&path).is_some() {
                count += 1;
                if let Ok(meta) = item.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(stamp) = modified.duration_since(std::time::UNIX_EPOCH) {
                            latest_mtime = latest_mtime.max(stamp.as_secs());
                        }
                    }
                }
                if covers.len() < MAX_COVERS {
                    covers.push(path);
                }
                if count >= MAX_COUNT {
                    break 'walk;
                }
            }
        }
    }

    if count == 0 {
        return None;
    }
    // 稳定"随机":按目录路径哈希挑选,重复打开时封面不变,缩略图缓存可复用。
    let cover = if covers.is_empty() {
        None
    } else {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        dir.hash(&mut hasher);
        Some(covers[(hasher.finish() as usize) % covers.len()].clone())
    };
    Some(FolderCard { dir: dir.to_path_buf(), name, photo_count: count, cover, latest_mtime })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_only_dirs_with_photos() {
        let root = std::env::temp_dir().join(format!("easyshow-folders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("trip/day1")).unwrap();
        std::fs::create_dir_all(root.join("empty")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        std::fs::write(root.join("trip/cover.png"), b"x").unwrap();
        std::fs::write(root.join("trip/day1/deep.png"), b"x").unwrap();
        std::fs::write(root.join("empty/readme.txt"), b"x").unwrap();
        std::fs::write(root.join(".hidden/x.png"), b"x").unwrap();

        let card = summarize(&root.join("trip"), "trip".into()).expect("trip has photos");
        assert_eq!(card.photo_count, 2);
        assert!(card.cover.is_some());
        assert!(summarize(&root.join("empty"), "empty".into()).is_none());
        assert!(summarize(&root.join(".hidden"), ".hidden".into()).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn depth_limit_is_respected() {
        let root = std::env::temp_dir().join(format!("easyshow-depth-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("a/b/c/d")).unwrap();
        std::fs::write(root.join("a/b/c/d/too-deep.png"), b"x").unwrap();

        // 超出 MAX_DEPTH 的照片不计入
        assert!(summarize(&root.join("a"), "a".into()).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
