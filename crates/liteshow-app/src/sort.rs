//! Directory scanning, image entries with metadata, and sorting.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use liteshow_core::from_extension;

/// One image file with the metadata needed for sorting and the info panel.
#[derive(Debug, Clone)]
pub struct ImageEntry {
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    /// Birth time when the platform/filesystem provides it (statx btime on
    /// Linux); `None` means "fall back to `modified`".
    pub created: Option<SystemTime>,
    /// Lowercase extension, for the "type" sort.
    pub ext: String,
}

impl ImageEntry {
    pub fn scan(path: PathBuf) -> Option<Self> {
        let meta = std::fs::metadata(&path).ok()?;
        if !meta.is_file() {
            return None;
        }
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        Some(Self {
            path,
            size: meta.len(),
            modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            created: meta.created().ok(),
            ext,
        })
    }

    /// Creation time with fallback to modification time.
    pub fn created_or_modified(&self) -> SystemTime {
        self.created.unwrap_or(self.modified)
    }
}

/// Scan a directory for supported image files (unordered; call [`sort_entries`]).
pub fn scan_entries(dir: &Path) -> Vec<ImageEntry> {
    let mut entries: Vec<ImageEntry> = Vec::new();
    if let Ok(read) = std::fs::read_dir(dir) {
        for item in read.flatten() {
            let path = item.path();
            if from_extension(&path).is_none() {
                continue;
            }
            if let Some(entry) = ImageEntry::scan(path) {
                entries.push(entry);
            }
        }
    }
    entries
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Modified,
    Created,
    Size,
    Type,
}

impl SortKey {
    /// Sensible default direction when switching to this key:
    /// dates read best newest-first, everything else ascending.
    pub fn default_ascending(self) -> bool {
        !matches!(self, SortKey::Modified | SortKey::Created)
    }
}

/// Sort entries in place; always falls back to the natural name order for ties.
pub fn sort_entries(entries: &mut [ImageEntry], key: SortKey, ascending: bool) {
    entries.sort_by(|a, b| {
        let ord = compare(a, b, key);
        let ord = if ascending { ord } else { ord.reverse() };
        ord.then_with(|| natural_cmp(&file_name(&a.path), &file_name(&b.path)))
    });
}

fn compare(a: &ImageEntry, b: &ImageEntry, key: SortKey) -> Ordering {
    match key {
        SortKey::Name => natural_cmp(&file_name(&a.path), &file_name(&b.path)),
        SortKey::Modified => a.modified.cmp(&b.modified),
        SortKey::Created => a.created_or_modified().cmp(&b.created_or_modified()),
        SortKey::Size => a.size.cmp(&b.size),
        SortKey::Type => a.ext.cmp(&b.ext).then_with(|| natural_cmp(&file_name(&a.path), &file_name(&b.path))),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Natural order: runs of digits compare numerically, everything else
/// case-insensitively (`IMG_2` < `IMG_10`).
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    let mut na = String::new();
                    while let Some(&c) = a.peek() {
                        if c.is_ascii_digit() {
                            na.push(c);
                            a.next();
                        } else {
                            break;
                        }
                    }
                    let mut nb = String::new();
                    while let Some(&c) = b.peek() {
                        if c.is_ascii_digit() {
                            nb.push(c);
                            b.next();
                        } else {
                            break;
                        }
                    }
                    let ta = na.trim_start_matches('0');
                    let tb = nb.trim_start_matches('0');
                    let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                    if ord != Ordering::Equal {
                        return ord;
                    }
                } else {
                    let ord = ca.to_ascii_lowercase().cmp(&cb.to_ascii_lowercase());
                    if ord != Ordering::Equal {
                        return ord;
                    }
                    a.next();
                    b.next();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn entry(name: &str, size: u64, secs: u64, created: Option<u64>, ext: &str) -> ImageEntry {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
        ImageEntry {
            path: PathBuf::from(name),
            size,
            modified: t,
            created: created.map(|s| SystemTime::UNIX_EPOCH + Duration::from_secs(s)),
            ext: ext.to_string(),
        }
    }

    #[test]
    fn natural_order_compares_digit_runs_numerically() {
        assert_eq!(natural_cmp("IMG_2.jpg", "IMG_10.jpg"), Ordering::Less);
        assert_eq!(natural_cmp("a9", "a10"), Ordering::Less);
        assert_eq!(natural_cmp("img_1.JPG", "IMG_01.jpg"), Ordering::Equal);
        assert_eq!(natural_cmp("b", "a10"), Ordering::Greater);
        assert_eq!(natural_cmp("abc", "abcd"), Ordering::Less);
        assert_eq!(natural_cmp("007", "7"), Ordering::Equal);
    }

    #[test]
    fn sorts_by_size_and_type() {
        let mut v = vec![
            entry("b.png", 300, 1, None, "png"),
            entry("a.jpg", 100, 2, None, "jpg"),
            entry("c.jpg", 200, 3, None, "jpg"),
        ];
        sort_entries(&mut v, SortKey::Size, true);
        assert_eq!(v[0].size, 100);
        assert_eq!(v[2].size, 300);

        sort_entries(&mut v, SortKey::Type, true);
        assert_eq!(v[0].ext, "jpg");
        assert_eq!(v[1].ext, "jpg");
        assert_eq!(v[2].ext, "png");
        assert_eq!(file_name(&v[0].path), "a.jpg"); // 同类型按自然名
    }

    #[test]
    fn created_falls_back_to_modified() {
        let mut v = vec![
            entry("old-no-btime.jpg", 1, 10, None, "jpg"), // created 缺失 → 用 modified=10
            entry("new.jpg", 1, 20, Some(30), "jpg"),
        ];
        sort_entries(&mut v, SortKey::Created, true);
        assert_eq!(file_name(&v[0].path), "old-no-btime.jpg");
        sort_entries(&mut v, SortKey::Created, false);
        assert_eq!(file_name(&v[0].path), "new.jpg");
    }

    #[test]
    fn name_sort_defaults_ascending_and_ties_are_stable_by_name() {
        let mut v = vec![
            entry("photo12.jpg", 1, 1, None, "jpg"),
            entry("photo2.jpg", 1, 1, None, "jpg"),
        ];
        sort_entries(&mut v, SortKey::Name, true);
        assert_eq!(file_name(&v[0].path), "photo2.jpg");
        assert!(SortKey::Name.default_ascending());
        assert!(!SortKey::Modified.default_ascending());
    }
}
