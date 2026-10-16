//! Left-side directory tree: lazy-loaded children, expand-to-path support.

use std::path::{Path, PathBuf};

use crate::icons;

/// 树节点图标:Windows 上优先用资源管理器原生图标(见 shell_icons),
/// 其他平台/提取失败时回退到图标字体的字形。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(not(windows), allow(dead_code))] // ThisPc/Drive/Glyph 仅在 Windows 组装树时构造
pub enum NodeIcon {
    Glyph(&'static str),
    ThisPc,
    Drive(char),
    Folder,
}

impl NodeIcon {
    /// 字形回退时使用的字符。
    pub fn fallback_glyph(self) -> &'static str {
        match self {
            NodeIcon::Glyph(g) => g,
            NodeIcon::ThisPc => icons::DESKTOP,
            NodeIcon::Drive(_) => icons::HARD_DRIVE,
            NodeIcon::Folder => icons::FOLDER,
        }
    }
}

pub struct TreeNode {
    pub path: PathBuf,
    pub name: String,
    /// 行首图标(原生图标或字形回退)。
    pub icon: NodeIcon,
    pub expanded: bool,
    loaded: bool,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    #[cfg_attr(windows, allow(dead_code))] // Windows 下由 app 组装「此电脑」根
    pub fn new_root(path: PathBuf) -> Self {
        Self {
            name: display_name(&path),
            path,
            icon: NodeIcon::Folder,
            expanded: true,
            loaded: false,
            children: Vec::new(),
        }
    }

    /// 通用构造(平台根、盘符、常用文件夹等)。
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn new_branch(
        name: String,
        path: PathBuf,
        icon: NodeIcon,
        expanded: bool,
        loaded: bool,
    ) -> Self {
        Self { name, path, icon, expanded, loaded, children: Vec::new() }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Load children (directories only, hidden ones skipped) on first use.
    pub fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                dirs.push(path);
            }
        }
        dirs.sort_by_cached_key(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default()
        });
        self.children = dirs
            .into_iter()
            .map(|path| TreeNode {
                name: display_name(&path),
                path,
                icon: NodeIcon::Folder,
                expanded: false,
                loaded: false,
                children: Vec::new(),
            })
            .collect();
    }

    /// Expand the tree down to `target` (loading intermediate levels).
    /// Returns true when `target` is inside this subtree.
    pub fn expand_to(&mut self, target: &Path) -> bool {
        if self.path == target {
            self.expanded = true;
            self.ensure_loaded();
            return true;
        }
        if !target.starts_with(&self.path) {
            return false;
        }
        self.expanded = true;
        self.ensure_loaded();
        for child in &mut self.children {
            if child.expand_to(target) {
                return true;
            }
        }
        false
    }

    /// Find a node by path in this subtree.
    pub fn find_mut(&mut self, target: &Path) -> Option<&mut TreeNode> {
        if self.path == target {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(target) {
                return Some(found);
            }
        }
        None
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
