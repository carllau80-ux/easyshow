//! Application state and the eframe glue.
//!
//! Two modes: a thumbnail **grid** for the selected directory, and a single
//! image **viewer**. A directory tree on the left, file/EXIF info on the right.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui;
use liteshow_core::{ExifSummary, ImageBuf};

use crate::decode::{DecodeEvent, DecodeService, Purpose};
use crate::i18n::{tr, Key, Lang};
use crate::sort::{scan_entries, sort_entries, ImageEntry, SortKey};
use crate::theme::{self, ThemeMode};
use crate::thumbs::ThumbService;
use crate::tree::TreeNode;
use crate::viewer::{Viewer, ZoomMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Grid,
    Viewer,
}

pub struct CurrentImage {
    pub path: PathBuf,
    /// As decoded (EXIF orientation applied, possibly downscaled for display).
    pub original: Arc<ImageBuf>,
    /// `original` with user rotation/flip applied; what gets uploaded.
    pub display: Arc<ImageBuf>,
    pub texture: egui::TextureHandle,
    /// True when `original` was resized down for display-size decoding.
    pub downscaled: bool,
}

/// Everything the right-hand info panel shows about one file.
pub struct InfoData {
    pub path: PathBuf,
    pub dims: Option<(u32, u32)>,
    pub file_size: Option<u64>,
    pub modified: Option<String>,
    pub format: Option<&'static str>,
    pub exif: Option<ExifSummary>,
}

pub struct LiteshowApp {
    pub lang: Lang,
    // --- directory tree ---
    pub tree_root: TreeNode,
    pub current_dir: PathBuf,
    // --- listing / grid ---
    pub entries: Vec<ImageEntry>,
    pub selected: usize,
    pub mode: Mode,
    pub grid_cols: usize,
    pub sort_key: SortKey,
    pub sort_asc: bool,
    // --- 输入行为 ---
    /// false(默认):滚轮缩放;true:滚轮切换上/下一张(Ctrl+滚轮仍为缩放)。
    pub wheel_navigates: bool,
    /// 每帧由输入层写入的滚轮"格"数(Line 事件=1,Point 事件按 50px=1 折算)。
    pub(crate) pending_wheel_units: f32,
    zoom_accum: f32,
    nav_accum: f32,
    // --- viewer ---
    pub viewer: Viewer,
    pub current: Option<CurrentImage>,
    pub loading: bool,
    pub full_requested: bool,
    pub error: Option<String>,
    decode: DecodeService,
    generation: u64,
    /// Decoded neighbors, ready to display instantly (path → (pixels, downscaled)).
    prefetch: HashMap<PathBuf, (Arc<ImageBuf>, bool)>,
    prefetch_order: VecDeque<PathBuf>,
    // --- thumbnails ---
    thumbs: ThumbService,
    pub thumb_imgs: HashMap<PathBuf, Arc<ImageBuf>>,
    pub thumb_tex: HashMap<PathBuf, egui::TextureHandle>,
    thumb_order: VecDeque<PathBuf>,
    thumb_inflight: HashSet<PathBuf>,
    thumb_generation: u64,
    // --- info panel ---
    pub info: Option<InfoData>,
    // --- ui ---
    pub canvas_size: egui::Vec2,
    pub fullscreen: bool,
    pub show_tree: bool,
    pub show_info: bool,
    pub theme_mode: ThemeMode,
    /// 资源管理器原生图标(仅 Windows;懒加载)。
    pub shell_icons: Option<crate::shell_icons::ShellIcons>,
    shell_icons_tried: bool,
}

/// 平台根节点:
/// - Windows:资源管理器式的「此电脑」→ 常用位置(桌面/下载/图片/文档)+ 各盘符(带卷标);
/// - 其他平台:以用户主目录为根。
fn build_tree_root(lang: Lang, home: &Path) -> TreeNode {
    #[cfg(windows)]
    {
        use crate::tree::NodeIcon;
        let _ = home;
        let mut root = TreeNode::new_branch(
            tr(lang, Key::ThisPC).to_string(),
            PathBuf::new(),
            NodeIcon::ThisPc,
            true,
            true,
        );
        // 常用位置(资源管理器"快速访问"的等价物)
        let profile = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        for (key, folder) in [
            (Key::FolderDesktop, "Desktop"),
            (Key::FolderDownloads, "Downloads"),
            (Key::FolderPictures, "Pictures"),
            (Key::FolderDocuments, "Documents"),
        ] {
            let path = profile.join(folder);
            if path.is_dir() {
                root.children.push(TreeNode::new_branch(
                    tr(lang, key).to_string(),
                    path,
                    NodeIcon::Folder,
                    false,
                    false,
                ));
            }
        }
        // 盘符:"卷标 (C:)" / "本地磁盘 (C:)"
        for letter in b'A'..=b'Z' {
            let letter = letter as char;
            let path = PathBuf::from(format!("{letter}:\\"));
            if !path.is_dir() {
                continue;
            }
            let label = volume_label(letter).unwrap_or_else(|| tr(lang, Key::LocalDisk).to_string());
            root.children.push(TreeNode::new_branch(
                format!("{label} ({letter}:)"),
                path,
                NodeIcon::Drive(letter),
                false,
                false,
            ));
        }
        root
    }
    #[cfg(not(windows))]
    {
        let _ = lang;
        TreeNode::new_root(home.to_path_buf())
    }
}

/// 读取盘符卷标(Windows)。失败时调用方回退到「本地磁盘」。
#[cfg(windows)]
fn volume_label(letter: char) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetVolumeInformationW(
            root_path: *const u16,
            volume_name: *mut u16,
            volume_name_size: u32,
            serial: *mut u32,
            max_component: *mut u32,
            flags: *mut u32,
            fs_name: *mut u16,
            fs_name_size: u32,
        ) -> i32;
    }

    let root: Vec<u16> = std::ffi::OsStr::new(&format!("{letter}:\\"))
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut buf = [0u16; 261];
    let ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            buf.as_mut_ptr(),
            buf.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    if ok == 0 {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    if len == 0 {
        None
    } else {
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}
/// 用户主目录:HOME → USERPROFILE → 平台兜底。
fn home_dir() -> PathBuf {
    for key in ["HOME", "USERPROFILE"] {
        if let Some(value) = std::env::var_os(key) {
            let path = PathBuf::from(value);
            if path.is_dir() {
                return path;
            }
        }
    }
    #[cfg(windows)]
    {
        PathBuf::from("C:\\")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/")
    }
}

/// Windows 的 `fs::canonicalize` 会加 `\\?\` 前缀,去掉它,
/// 让路径能与盘符/树节点正常前缀匹配。
fn normalize_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(s) = path.to_str() {
            if let Some(rest) = s.strip_prefix(r"\\?\") {
                return PathBuf::from(rest);
            }
        }
    }
    path
}

const PREFETCH_CAP: usize = 4;
const THUMB_CACHE_CAP: usize = 512;

impl LiteshowApp {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let lang = Lang::detect();
        let home = home_dir();
        let mut app = Self {
            lang,
            tree_root: build_tree_root(lang, &home),
            current_dir: home.clone(),
            entries: Vec::new(),
            selected: 0,
            mode: Mode::Grid,
            grid_cols: 4,
            sort_key: SortKey::Name,
            sort_asc: true,
            wheel_navigates: false,
            pending_wheel_units: 0.0,
            zoom_accum: 0.0,
            nav_accum: 0.0,
            viewer: Viewer::default(),
            current: None,
            loading: false,
            full_requested: false,
            error: None,
            decode: DecodeService::new(3),
            generation: 0,
            prefetch: HashMap::new(),
            prefetch_order: VecDeque::new(),
            thumbs: ThumbService::new(2),
            thumb_imgs: HashMap::new(),
            thumb_tex: HashMap::new(),
            thumb_order: VecDeque::new(),
            thumb_inflight: HashSet::new(),
            thumb_generation: 0,
            info: None,
            canvas_size: egui::vec2(1000.0, 700.0),
            fullscreen: false,
            show_tree: true,
            show_info: true,
            theme_mode: ThemeMode::System,
            shell_icons: None,
            shell_icons_tried: false,
        };
        match path {
            Some(p) if p.is_file() => app.open_path(&cc.egui_ctx, p),
            Some(p) if p.is_dir() => app.open_path(&cc.egui_ctx, p),
            _ => {
                app.tree_root.expand_to(&home);
                app.open_dir(&cc.egui_ctx, home, None, Mode::Grid);
            }
        }
        app
    }

    // ------------------------------------------------------------------
    // Opening / navigation
    // ------------------------------------------------------------------

    pub fn open_path(&mut self, ctx: &egui::Context, path: PathBuf) {
        // 规范化(绝对路径 + 解析符号链接),树定位与目录比较都依赖它
        let path = normalize_path(std::fs::canonicalize(&path).unwrap_or(path));
        if path.is_dir() {
            self.open_dir(ctx, path, None, Mode::Grid);
        } else if path.is_file() {
            let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
            self.open_dir(ctx, dir, Some(path), Mode::Viewer);
        } else {
            self.error = Some(format!("{}: not found", path.display()));
        }
    }

    pub fn open_dir(&mut self, ctx: &egui::Context, dir: PathBuf, select: Option<PathBuf>, mode: Mode) {
        let dir = normalize_path(std::fs::canonicalize(&dir).unwrap_or(dir));
        self.current_dir = dir.clone();
        self.entries = scan_entries(&dir);
        sort_entries(&mut self.entries, self.sort_key, self.sort_asc);
        self.selected = select
            .and_then(|sel| self.entries.iter().position(|e| e.path == sel))
            .unwrap_or(0);
        if self.selected >= self.entries.len() {
            self.selected = 0;
        }
        self.thumb_generation += 1;
        self.thumb_inflight.clear();
        self.mode = mode;
        self.error = None;
        self.tree_root.expand_to(&dir);
        self.refresh_info();
        let title = self
            .selected_path()
            .map(|p| file_name(p))
            .unwrap_or_else(|| file_name(&dir));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{title} — Liteshow")));
        if mode == Mode::Viewer {
            self.load_current(ctx);
        } else {
            self.current = None; // 释放大纹理;网格接管
        }
    }

    pub fn open_selected_in_viewer(&mut self, ctx: &egui::Context) {
        if self.entries.is_empty() {
            return;
        }
        self.mode = Mode::Viewer;
        self.load_current(ctx);
    }

    pub fn exit_to_grid(&mut self) {
        self.mode = Mode::Grid;
        self.refresh_info();
    }

    pub fn select(&mut self, index: usize) {
        if index < self.entries.len() {
            self.selected = index;
            self.refresh_info();
        }
    }

    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.entries.get(self.selected).map(|e| &e.path)
    }

    fn path_at(&self, index: usize) -> Option<PathBuf> {
        self.entries.get(index).map(|e| e.path.clone())
    }

    // ------------------------------------------------------------------
    // Sorting
    // ------------------------------------------------------------------

    /// 选择排序键;对同一个键再次调用则切换升/降序。
    pub fn set_sort(&mut self, key: SortKey) {
        if self.sort_key == key {
            self.sort_asc = !self.sort_asc;
        } else {
            self.sort_key = key;
            self.sort_asc = key.default_ascending();
        }
        self.apply_sort_keep_selection();
    }

    pub fn toggle_sort_direction(&mut self) {
        self.sort_asc = !self.sort_asc;
        self.apply_sort_keep_selection();
    }

    // ------------------------------------------------------------------
    // Theme / language
    // ------------------------------------------------------------------

    pub fn set_theme_mode(&mut self, ctx: &egui::Context, mode: ThemeMode) {
        self.theme_mode = mode;
        theme::apply_preference(ctx, mode);
    }

    /// 语言切换后刷新树的平台根名称(Windows 的「此电脑」)。
    pub fn relabel_tree_root(&mut self) {
        if cfg!(windows) {
            self.tree_root.set_name(tr(self.lang, Key::ThisPC));
        }
    }

    fn apply_sort_keep_selection(&mut self) {
        let keep = self.selected_path().cloned();
        sort_entries(&mut self.entries, self.sort_key, self.sort_asc);
        if let Some(path) = keep {
            if let Some(position) = self.entries.iter().position(|e| e.path == path) {
                self.selected = position;
            }
        }
        self.refresh_info();
    }

    pub fn load_current(&mut self, ctx: &egui::Context) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = self.selected.min(self.entries.len() - 1);
        let Some(path) = self.path_at(self.selected) else { return };
        self.refresh_info();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{} — Liteshow", file_name(&path))));

        // Same image already displayed (e.g. re-entering the viewer): reuse.
        if let Some(current) = &self.current {
            if current.path == path && !self.loading {
                self.refit_if_needed();
                return;
            }
        }

        self.generation += 1;
        self.full_requested = false;
        self.error = None;

        if let Some((image, downscaled)) = self.prefetch.remove(&path) {
            // 预取命中:零解码,直接上屏
            let display = rotate_flip(&image, self.viewer.rotation, self.viewer.flip_h);
            let texture = upload_texture(ctx, &display);
            self.current = Some(CurrentImage { path: path.clone(), original: image, display, texture, downscaled });
            self.loading = false;
            self.refit_if_needed();
        } else {
            self.loading = true;
            let cap = self.decode_cap(ctx);
            self.decode.request(path.clone(), self.generation, Some(cap), Purpose::Current);
        }
        self.prefetch_neighbors(ctx);
    }

    pub fn next(&mut self, ctx: &egui::Context) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.entries.len();
        if self.mode == Mode::Viewer {
            self.load_current(ctx);
        } else {
            self.refresh_info();
        }
    }

    pub fn prev(&mut self, ctx: &egui::Context) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = (self.selected + self.entries.len() - 1) % self.entries.len();
        if self.mode == Mode::Viewer {
            self.load_current(ctx);
        } else {
            self.refresh_info();
        }
    }

    pub fn first(&mut self, ctx: &egui::Context) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = 0;
        if self.mode == Mode::Viewer {
            self.load_current(ctx);
        } else {
            self.refresh_info();
        }
    }

    pub fn last(&mut self, ctx: &egui::Context) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = self.entries.len() - 1;
        if self.mode == Mode::Viewer {
            self.load_current(ctx);
        } else {
            self.refresh_info();
        }
    }

    // ------------------------------------------------------------------
    // View actions (viewer)
    // ------------------------------------------------------------------

    pub fn set_fit(&mut self) {
        self.viewer.mode = ZoomMode::Fit;
        self.refit_if_needed();
    }

    pub fn set_actual(&mut self) {
        self.viewer.mode = ZoomMode::Fixed(1.0);
        self.viewer.zoom = 1.0;
        self.viewer.offset = egui::Vec2::ZERO;
        self.maybe_request_full_res();
    }

    pub fn zoom_step(&mut self, factor: f32) {
        let center = self.canvas_size / 2.0;
        let anchor = egui::pos2(center.x, center.y);
        let new_zoom = (self.viewer.zoom * factor).clamp(0.05, 8.0);
        self.viewer.zoom_at(new_zoom, anchor, anchor);
        self.viewer.mode = ZoomMode::Fixed(new_zoom);
        self.maybe_request_full_res();
    }

    pub fn action_rotate(&mut self, ctx: &egui::Context) {
        self.viewer.rotation = (self.viewer.rotation + 1) % 4;
        self.rebuild_display(ctx);
    }

    pub fn action_flip(&mut self, ctx: &egui::Context) {
        self.viewer.flip_h = !self.viewer.flip_h;
        self.rebuild_display(ctx);
    }

    pub fn set_fullscreen(&mut self, ctx: &egui::Context, on: bool) {
        self.fullscreen = on;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
    }

    pub fn zoom_label(&self) -> String {
        match self.viewer.mode {
            ZoomMode::Fit => tr(self.lang, Key::Fit).to_string(),
            ZoomMode::Fixed(_) => format!("{:.0}%", self.viewer.zoom * 100.0),
        }
    }

    /// Re-decode at full resolution once the user zooms past display size.
    pub(crate) fn maybe_request_full_res(&mut self) {
        let Some(current) = &self.current else { return };
        if current.downscaled && !self.full_requested && self.viewer.zoom > 1.25 {
            let path = current.path.clone();
            self.full_requested = true;
            self.generation += 1;
            self.loading = true;
            self.decode.request(path, self.generation, None, Purpose::Current);
        }
    }

    fn refit_if_needed(&mut self) {
        if self.viewer.mode != ZoomMode::Fit {
            return;
        }
        if let Some(current) = &self.current {
            let size = display_size(current);
            self.viewer.recompute_fit(size, self.canvas_size);
        }
    }

    fn rebuild_display(&mut self, ctx: &egui::Context) {
        let Some(original) = self.current.as_ref().map(|c| c.original.clone()) else { return };
        let display = rotate_flip(&original, self.viewer.rotation, self.viewer.flip_h);
        let texture = upload_texture(ctx, &display);
        if let Some(current) = self.current.as_mut() {
            current.display = display;
            current.texture = texture;
        }
        self.refit_if_needed();
    }

    fn decode_cap(&self, ctx: &egui::Context) -> (u32, u32) {
        let dpr = ctx.pixels_per_point();
        let cap = self.canvas_size * 2.0 * dpr;
        (cap.x.max(64.0) as u32, cap.y.max(64.0) as u32)
    }

    /// Decode the previous/next images ahead of time for instant navigation.
    fn prefetch_neighbors(&mut self, ctx: &egui::Context) {
        let n = self.entries.len();
        if n < 2 {
            return;
        }
        let cap = self.decode_cap(ctx);
        let current = self.path_at(self.selected);
        for delta in [1usize, n - 1] {
            let Some(path) = self.path_at((self.selected + delta) % n) else { continue };
            if Some(&path) == current.as_ref() || self.prefetch.contains_key(&path) {
                continue;
            }
            self.decode.request(path, self.generation, Some(cap), Purpose::Prefetch);
        }
    }

    // ------------------------------------------------------------------
    // Info panel
    // ------------------------------------------------------------------

    fn refresh_info(&mut self) {
        let Some(path) = self.path_at(self.selected) else {
            self.info = None;
            return;
        };
        if self.info.as_ref().map(|i| &i.path) == Some(&path) {
            return;
        }
        let meta = std::fs::metadata(&path).ok();
        let dims = liteshow_core::image_dimensions(&path).ok();
        let exif = liteshow_core::exif::read_summary(&path);
        let format = liteshow_core::codec::detect_format(&path).map(|f| f.name());
        self.info = Some(InfoData {
            path,
            dims,
            file_size: meta.as_ref().map(|m| m.len()),
            modified: meta.and_then(|m| m.modified().ok()).map(format_time),
            format,
            exif,
        });
    }

    // ------------------------------------------------------------------
    // Events
    // ------------------------------------------------------------------

    fn poll_decode(&mut self, ctx: &egui::Context) {
        while let Some(event) = self.decode.try_recv() {
            match event {
                DecodeEvent::Done(done) => {
                    if done.generation != self.generation {
                        continue;
                    }
                    match done.purpose {
                        Purpose::Current => {
                            let display = rotate_flip(&done.image, self.viewer.rotation, self.viewer.flip_h);
                            let texture = upload_texture(ctx, &display);
                            self.current = Some(CurrentImage {
                                path: done.path,
                                original: done.image,
                                display,
                                texture,
                                downscaled: done.downscaled,
                            });
                            self.loading = false;
                            self.error = None;
                            self.refit_if_needed();
                        }
                        Purpose::Prefetch => {
                            self.prefetch.insert(done.path.clone(), (done.image, done.downscaled));
                            self.prefetch_order.push_back(done.path);
                            while self.prefetch_order.len() > PREFETCH_CAP {
                                if let Some(old) = self.prefetch_order.pop_front() {
                                    self.prefetch.remove(&old);
                                }
                            }
                        }
                    }
                }
                DecodeEvent::Failed { generation, path, message, purpose } => {
                    if generation != self.generation || purpose != Purpose::Current {
                        continue;
                    }
                    self.loading = false;
                    self.error = Some(format!("{}: {message}", file_name(&path)));
                }
            }
        }
    }

    fn poll_thumbs(&mut self) {
        while let Some(event) = self.thumbs.try_recv() {
            self.thumb_inflight.remove(&event.path);
            if event.generation != self.thumb_generation {
                continue;
            }
            if let Ok(image) = event.result {
                self.thumb_imgs.insert(event.path.clone(), image);
                self.thumb_order.push_back(event.path);
                while self.thumb_order.len() > THUMB_CACHE_CAP {
                    if let Some(old) = self.thumb_order.pop_front() {
                        self.thumb_imgs.remove(&old);
                        self.thumb_tex.remove(&old);
                    }
                }
            }
        }
    }

    /// Queue a thumbnail for `path` unless present / already in flight.
    pub fn request_thumb(&mut self, path: &Path) {
        if self.thumb_imgs.contains_key(path) || self.thumb_inflight.contains(path) {
            return;
        }
        self.thumb_inflight.insert(path.to_path_buf());
        self.thumbs.request(path.to_path_buf(), self.thumb_generation);
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        use egui::Key;
        let (right, left, up, down, home, end, enter, escape, zoom_in, zoom_out, fit_key, actual_key, rotate_key, flip_key, full_key) =
            ctx.input(|i| {
                (
                    i.key_pressed(Key::ArrowRight),
                    i.key_pressed(Key::ArrowLeft),
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::Home),
                    i.key_pressed(Key::End),
                    i.key_pressed(Key::Enter),
                    i.key_pressed(Key::Escape),
                    i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals),
                    i.key_pressed(Key::Minus),
                    i.key_pressed(Key::Num0),
                    i.key_pressed(Key::Num1),
                    i.key_pressed(Key::R),
                    i.key_pressed(Key::F),
                    i.key_pressed(Key::F11),
                )
            });

        match self.mode {
            Mode::Grid => {
                let cols = self.grid_cols.max(1);
                if right {
                    self.select_relative(1);
                }
                if left {
                    self.select_relative(-1);
                }
                if down {
                    self.select_relative(cols as isize);
                }
                if up {
                    self.select_relative(-(cols as isize));
                }
                if home {
                    self.select(0);
                }
                if end {
                    self.select(self.entries.len().saturating_sub(1));
                }
                if enter {
                    self.open_selected_in_viewer(ctx);
                }
            }
            Mode::Viewer => {
                if right {
                    self.next(ctx);
                }
                if left {
                    self.prev(ctx);
                }
                if home {
                    self.first(ctx);
                }
                if end {
                    self.last(ctx);
                }
                if zoom_in {
                    self.zoom_step(1.15);
                }
                if zoom_out {
                    self.zoom_step(1.0 / 1.15);
                }
                if fit_key {
                    self.set_fit();
                }
                if actual_key {
                    self.set_actual();
                }
                if rotate_key {
                    self.action_rotate(ctx);
                }
                if flip_key {
                    self.action_flip(ctx);
                }
                if escape {
                    if self.fullscreen {
                        self.set_fullscreen(ctx, false);
                    } else {
                        self.exit_to_grid();
                    }
                }
            }
        }
        if full_key {
            let on = !self.fullscreen;
            self.set_fullscreen(ctx, on);
        }
    }

    /// 滚轮缩放:按"格"计量,每格固定 1.15×,以光标为锚。
    /// 不依赖 egui 的平滑滚动量,行为在真实鼠标 / 触控板 / 各平台上一致。
    pub(crate) fn zoom_wheel(&mut self, units: f32, anchor: egui::Pos2, center: egui::Pos2) {
        self.zoom_accum += units;
        let steps = self.zoom_accum.trunc().clamp(-10.0, 10.0);
        if steps == 0.0 {
            return;
        }
        self.zoom_accum -= steps;
        let factor = 1.15f32.powf(steps);
        let new_zoom = (self.viewer.zoom * factor).clamp(0.05, 8.0);
        self.viewer.zoom_at(new_zoom, anchor, center);
        self.viewer.mode = ZoomMode::Fixed(new_zoom);
        self.maybe_request_full_res();
    }

    /// 滚轮翻页模式:每"格"翻一张。
    pub(crate) fn navigate_wheel(&mut self, ctx: &egui::Context, units: f32) {
        self.nav_accum += units;
        let steps = self.nav_accum.trunc().clamp(-20.0, 20.0);
        if steps == 0.0 {
            return;
        }
        self.nav_accum -= steps;
        for _ in 0..(steps as i32).unsigned_abs() {
            if steps > 0.0 {
                self.prev(ctx);
            } else {
                self.next(ctx);
            }
        }
    }

    fn select_relative(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let n = self.entries.len() as isize;
        let next = (self.selected as isize + delta).clamp(0, n - 1);
        self.select(next as usize);
    }
}

impl eframe::App for LiteshowApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // 资源管理器原生图标(仅 Windows,首帧懒加载一次)
        if !self.shell_icons_tried {
            self.shell_icons_tried = true;
            self.shell_icons = crate::shell_icons::load(&ctx);
        }

        // Drag & drop.
        let dropped: Option<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().find_map(|f| {
                let path = f.path().to_path_buf();
                if path.as_os_str().is_empty() {
                    None
                } else {
                    Some(path)
                }
            })
        });
        if let Some(path) = dropped {
            self.open_path(&ctx, path);
        }

        self.poll_decode(&ctx);
        self.poll_thumbs();
        self.handle_keys(&ctx);

        // 滚轮输入:在本帧、面板消费事件之前读取;由画布(悬停时)取用。
        // 单位归一化:Line=1 格;Page=±1 格;Point(触控板等高精度)按 50px 折 1 格。
        self.pending_wheel_units = ctx.input(|i| {
            let mut units = 0.0f32;
            for event in &i.events {
                if let egui::Event::MouseWheel { unit, delta, .. } = event {
                    units += match unit {
                        egui::MouseWheelUnit::Line => delta.y,
                        egui::MouseWheelUnit::Page => delta.y.signum(),
                        egui::MouseWheelUnit::Point => delta.y / 50.0,
                    };
                }
            }
            units
        });

        egui::Panel::top("liteshow-toolbar").show(ui, |ui| crate::ui::toolbar(self, ui));
        egui::Panel::bottom("liteshow-status").show(ui, |ui| crate::ui::status(self, ui));
        if self.show_tree {
            egui::Panel::left("liteshow-tree")
                .default_size(215.0)
                .size_range(150.0..=460.0)
                .resizable(true)
                .show(ui, |ui| crate::ui::tree_panel(self, ui));
        }
        if self.show_info {
            egui::Panel::right("liteshow-info")
                .default_size(268.0)
                .size_range(200.0..=420.0)
                .resizable(true)
                .show(ui, |ui| crate::ui::info_panel(self, ui));
        }
        egui::CentralPanel::no_frame().show(ui, |ui| match self.mode {
            Mode::Grid => crate::ui::grid(self, ui),
            Mode::Viewer => crate::ui::canvas(self, ui),
        });

        // eframe is event-driven: keep frames coming while work is in flight.
        if self.loading || !self.thumb_inflight.is_empty() {
            ctx.request_repaint();
        }
    }
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub fn display_size(current: &CurrentImage) -> egui::Vec2 {
    egui::vec2(current.display.width as f32, current.display.height as f32)
}

fn rotate_flip(src: &ImageBuf, turns: u8, flip_h: bool) -> Arc<ImageBuf> {
    let mut buf = if turns % 4 == 0 { src.clone() } else { src.clone().rotated(turns % 4) };
    if flip_h {
        buf = buf.flip_horizontal();
    }
    Arc::new(buf)
}

fn upload_texture(ctx: &egui::Context, image: &ImageBuf) -> egui::TextureHandle {
    let color =
        egui::ColorImage::from_rgba_unmultiplied([image.width as usize, image.height as usize], &image.rgba);
    ctx.load_texture("liteshow-image", color, egui::TextureOptions::LINEAR)
}

pub fn upload_thumb_texture(ctx: &egui::Context, image: &ImageBuf) -> egui::TextureHandle {
    let color =
        egui::ColorImage::from_rgba_unmultiplied([image.width as usize, image.height as usize], &image.rgba);
    ctx.load_texture("liteshow-thumb", color, egui::TextureOptions::LINEAR)
}

pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bytes as f64;
    if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

/// Local-time "YYYY-MM-DD HH:MM".
fn format_time(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    #[cfg(unix)]
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        let time = secs as libc::time_t;
        if !libc::localtime_r(&time, &mut tm).is_null() {
            return format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                tm.tm_year + 1900,
                tm.tm_mon + 1,
                tm.tm_mday,
                tm.tm_hour,
                tm.tm_min
            );
        }
    }
    civil_utc(secs)
}

#[cfg(not(unix))]
fn civil_utc(secs: i64) -> String {
    civil_utc_impl(secs)
}

#[cfg(unix)]
fn civil_utc(secs: i64) -> String {
    civil_utc_impl(secs)
}

fn civil_utc_impl(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rem / 3600, (rem % 3600) / 60)
}

/// Howard Hinnant's civil-from-days algorithm (proleptic Gregorian calendar).
#[allow(dead_code)]
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
