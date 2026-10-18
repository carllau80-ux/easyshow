//! Toolbar (Apple 风格图标), directory tree, thumbnail grid, viewer canvas,
//! info panel and status bar.

use eframe::egui;

use crate::app::{display_size, file_name, human_size, upload_thumb_texture, EasyShowApp, Mode};
use crate::i18n::{tr, Key, Lang};
use crate::icons;
use crate::sort::{ImageEntry, SortKey};
use crate::theme::{Palette, ThemeMode};
use crate::tree::TreeNode;
use crate::viewer::ZoomMode;

// ----------------------------------------------------------------------
// 图标按钮小工具
// ----------------------------------------------------------------------

fn icon_text(icon: &str) -> egui::RichText {
    egui::RichText::new(icon).size(17.0)
}

fn icon_button(ui: &mut egui::Ui, icon: &str, tooltip: &str, enabled: bool) -> egui::Response {
    let button = egui::Button::new(icon_text(icon))
        .min_size(egui::vec2(30.0, 28.0))
        .frame(false);
    ui.add_enabled(enabled, button).on_hover_text(tooltip)
}

fn icon_toggle(ui: &mut egui::Ui, icon: &str, tooltip: &str, active: bool) -> egui::Response {
    let color = if active { Palette::for_ui(ui).accent } else { ui.visuals().text_color() };
    let button = egui::Button::new(egui::RichText::new(icon).size(17.0).color(color))
        .min_size(egui::vec2(30.0, 28.0))
        .frame(false);
    ui.add(button).on_hover_text(tooltip)
}

fn toolbar_sep(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);
}

fn sort_label(lang: Lang, key: SortKey) -> &'static str {
    match key {
        SortKey::Name => tr(lang, Key::ByName),
        SortKey::Taken => tr(lang, Key::ByTaken),
        SortKey::Modified => tr(lang, Key::ByModified),
        SortKey::Created => tr(lang, Key::ByCreated),
        SortKey::Size => tr(lang, Key::BySize),
        SortKey::Type => tr(lang, Key::ByType),
    }
}

// ----------------------------------------------------------------------
// Toolbar
// ----------------------------------------------------------------------

pub fn toolbar(app: &mut EasyShowApp, ui: &mut egui::Ui) {
    let lang = app.lang;
    let mut pick_sort: Option<SortKey> = None;
    let mut flip_sort_dir = false;
    let mut pick_lang: Option<Lang> = None;
    let mut pick_theme: Option<ThemeMode> = None;
    let mut pick_interval: Option<u32> = None;

    ui.horizontal(|ui| {
        ui.add_space(6.0);
        if icon_button(ui, icons::FILE_IMAGE, tr(lang, Key::Open), true).clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                let ctx = ui.ctx().clone();
                app.open_path(&ctx, path);
            }
        }
        if icon_button(ui, icons::FOLDER_OPEN, tr(lang, Key::OpenFolder), true).clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                let ctx = ui.ctx().clone();
                app.open_dir(&ctx, dir, None, Mode::Grid);
            }
        }
        toolbar_sep(ui);

        if icon_toggle(ui, icons::TREE_STRUCTURE, tr(lang, Key::Tree), app.show_tree).clicked() {
            app.show_tree = !app.show_tree;
        }
        if icon_toggle(ui, icons::INFO, tr(lang, Key::InfoPanel), app.show_info).clicked() {
            app.show_info = !app.show_info;
        }
        toolbar_sep(ui);

        // 地图模式切换(网格 ↔ 地图)
        if app.mode == Mode::Map {
            if icon_toggle(ui, icons::MAP_TRIFOLD, tr(lang, Key::BackToGrid), true).clicked() {
                app.exit_map_to_grid(ui.ctx());
            }
        } else if app.mode == Mode::Grid {
            let map_resp =
                icon_button(ui, icons::MAP_TRIFOLD, tr(lang, Key::Map), !app.entries.is_empty());
            if map_resp.clicked() {
                app.enter_map(ui.ctx());
            }
        }
        toolbar_sep(ui);

        let has_images = !app.entries.is_empty();
        if icon_button(ui, icons::CARET_LEFT, tr(lang, Key::Prev), has_images).clicked() {
            app.prev(ui.ctx());
        }
        if icon_button(ui, icons::CARET_RIGHT, tr(lang, Key::Next), has_images).clicked() {
            app.next(ui.ctx());
        }

        if app.mode == Mode::Viewer {
            toolbar_sep(ui);
            if icon_button(ui, icons::SQUARES_FOUR, tr(lang, Key::BackToGrid), true).clicked() {
                app.exit_to_grid();
            }
            toolbar_sep(ui);
            if icon_button(ui, icons::MAGNIFYING_GLASS_MINUS, tr(lang, Key::ZoomOut), true).clicked() {
                app.zoom_step(1.0 / 1.15);
            }
            ui.label(egui::RichText::new(app.zoom_label()).size(12.5));
            if icon_button(ui, icons::MAGNIFYING_GLASS_PLUS, tr(lang, Key::ZoomIn), true).clicked() {
                app.zoom_step(1.15);
            }
            toolbar_sep(ui);
            if icon_button(ui, icons::CORNERS_OUT, tr(lang, Key::Fit), true).clicked() {
                app.set_fit();
            }
            if icon_button(ui, icons::CORNERS_IN, tr(lang, Key::ActualSize), true).clicked() {
                app.set_actual();
            }
            toolbar_sep(ui);
            if icon_button(ui, icons::ARROW_CLOCKWISE, tr(lang, Key::Rotate), true).clicked() {
                app.action_rotate(ui.ctx());
            }
            if icon_button(ui, icons::FLIP_HORIZONTAL, tr(lang, Key::Flip), true).clicked() {
                app.action_flip(ui.ctx());
            }
        }

        // 动图播放/暂停(GIF / WebP,完整版)
        #[cfg(feature = "formats")]
        if app.mode == Mode::Viewer && app.anim_active() {
            toolbar_sep(ui);
            let (icon, tip) = if app.anim_playing() {
                (icons::PAUSE, tr(lang, Key::Pause))
            } else {
                (icons::PLAY, tr(lang, Key::Play))
            };
            if icon_button(ui, icon, tip, true).clicked() {
                app.toggle_anim(ui.ctx());
            }
            let frames = app.anim_frames();
            if frames > 0 {
                ui.weak(format!("{frames} {}", tr(lang, Key::Frames)));
            }
        }

        if app.mode == Mode::Map {
            toolbar_sep(ui);
            if icon_button(ui, icons::MAGNIFYING_GLASS_MINUS, tr(lang, Key::ZoomOut), true).clicked() {
                app.zoom_map_steps(-1.0);
            }
            if icon_button(ui, icons::MAGNIFYING_GLASS_PLUS, tr(lang, Key::ZoomIn), true).clicked() {
                app.zoom_map_steps(1.0);
            }
            if icon_button(ui, icons::CORNERS_OUT, tr(lang, Key::FitPhotos), true).clicked() {
                app.fit_map_to_photos();
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (fullscreen_icon, fullscreen_tip) = if app.fullscreen {
                (icons::ARROWS_IN, tr(lang, Key::ExitFullscreen))
            } else {
                (icons::ARROWS_OUT, tr(lang, Key::Fullscreen))
            };
            if icon_button(ui, fullscreen_icon, fullscreen_tip, true).clicked() {
                let on = !app.fullscreen;
                app.set_fullscreen(ui.ctx(), on);
            }

            ui.menu_button(icon_text(icons::GEAR), |ui| {
                ui.set_min_width(190.0);
                ui.label(egui::RichText::new(tr(lang, Key::Theme)).strong());
                if ui.radio(app.theme_mode == ThemeMode::System, tr(lang, Key::ThemeSystem)).clicked() {
                    pick_theme = Some(ThemeMode::System);
                }
                if ui.radio(app.theme_mode == ThemeMode::Light, tr(lang, Key::ThemeLight)).clicked() {
                    pick_theme = Some(ThemeMode::Light);
                }
                if ui.radio(app.theme_mode == ThemeMode::Dark, tr(lang, Key::ThemeDark)).clicked() {
                    pick_theme = Some(ThemeMode::Dark);
                }
                ui.separator();
                ui.label(egui::RichText::new(tr(lang, Key::WheelMode)).strong());
                ui.radio_value(&mut app.wheel_navigates, false, tr(lang, Key::WheelZoom));
                ui.radio_value(&mut app.wheel_navigates, true, tr(lang, Key::WheelNavigate));
                ui.separator();
                ui.checkbox(&mut app.delete_enabled, tr(lang, Key::AllowDelete));
                ui.separator();
                ui.label(egui::RichText::new(tr(lang, Key::SlideshowInterval)).strong());
                for secs in [3u32, 5, 10] {
                    if ui
                        .radio(
                            app.slideshow_interval_secs == secs,
                            format!("{secs} {}", tr(lang, Key::Seconds)),
                        )
                        .clicked()
                    {
                        pick_interval = Some(secs);
                    }
                }
                ui.separator();
                let mut online = app.net.online;
                if ui.checkbox(&mut online, tr(lang, Key::EnableOnline)).changed() {
                    app.set_online(ui.ctx(), online);
                }
                ui.small(tr(lang, Key::OnlinePrivacy));
                if app.net.online {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(tr(lang, Key::AmapKey));
                        let mut key = app.net.amap_key.clone();
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut key)
                                .desired_width(150.0)
                                .hint_text(tr(lang, Key::AmapKeyHint)),
                        );
                        if response.changed() {
                            app.set_amap_key(key);
                        }
                    });
                    ui.small(tr(lang, Key::AmapKeyHelp));
                    ui.label(tr(lang, Key::TileSource));
                    for (source, label) in [
                        (crate::config::TileSource::Auto, Key::TileAuto),
                        (crate::config::TileSource::Amap, Key::TileAmap),
                        (crate::config::TileSource::Osm, Key::TileOsm),
                    ] {
                        if ui.radio(app.net.tile_source == source, tr(lang, label)).clicked() {
                            app.set_tile_source(source);
                        }
                    }
                }
                ui.separator();
                ui.weak(format!("{} v{}", crate::APP_NAME, env!("CARGO_PKG_VERSION")));
            })
            .response
            .on_hover_text(tr(lang, Key::Settings));

            ui.menu_button(icon_text(icons::TRANSLATE), |ui| {
                ui.set_min_width(130.0);
                if ui.selectable_label(app.lang == Lang::Zh, "中文").clicked() {
                    pick_lang = Some(Lang::Zh);
                }
                if ui.selectable_label(app.lang == Lang::En, "English").clicked() {
                    pick_lang = Some(Lang::En);
                }
            })
            .response
            .on_hover_text(tr(lang, Key::Language));

            ui.menu_button(icon_text(icons::ARROWS_DOWN_UP), |ui| {
                ui.set_min_width(190.0);
                for key in [SortKey::Taken, SortKey::Name, SortKey::Modified, SortKey::Created, SortKey::Size, SortKey::Type] {
                    let label = sort_label(lang, key);
                    let text = if app.sort_key == key {
                        format!("{}  {label}", icons::CHECK)
                    } else {
                        format!("    {label}")
                    };
                    if ui.selectable_label(app.sort_key == key, text).clicked() {
                        pick_sort = Some(key);
                    }
                }
                ui.separator();
                let (dir_icon, dir_label) = if app.sort_asc {
                    (icons::ARROW_UP, tr(lang, Key::Ascending))
                } else {
                    (icons::ARROW_DOWN, tr(lang, Key::Descending))
                };
                if ui.button(format!("{dir_icon}  {dir_label}")).clicked() {
                    flip_sort_dir = true;
                }
            })
            .response
            .on_hover_text(tr(lang, Key::Sort));

            // 幻灯片(整目录或所选批量)
            let slide_on = app.is_slideshow();
            if icon_toggle(ui, icons::SLIDESHOW, tr(lang, Key::Slideshow), slide_on).clicked() {
                if slide_on {
                    app.stop_slideshow();
                } else {
                    app.start_slideshow(ui.ctx());
                }
            }

            // 删除(默认只读;未开启时禁用)
            let has_target = ((app.mode == Mode::Grid || app.mode == Mode::Map)
                && !app.selection.is_empty())
                || (app.mode == Mode::Viewer && app.current.is_some());
            if icon_button(ui, icons::TRASH, tr(lang, Key::Delete), app.delete_enabled && has_target)
                .clicked()
            {
                app.request_delete();
            }
        });
    });

    if let Some(key) = pick_sort {
        app.set_sort(key);
    }
    if flip_sort_dir {
        app.toggle_sort_direction();
    }
    if let Some(lang) = pick_lang {
        app.set_language(lang);
    }
    if let Some(mode) = pick_theme {
        app.set_theme_mode(ui.ctx(), mode);
    }
    if let Some(secs) = pick_interval {
        app.slideshow_interval_secs = secs;
    }
}

// ----------------------------------------------------------------------
// Directory tree (left)
// ----------------------------------------------------------------------

enum TreeAction {
    Toggle(std::path::PathBuf),
    Select(std::path::PathBuf),
}

pub fn tree_panel(app: &mut EasyShowApp, ui: &mut egui::Ui) {
    ui.add_space(6.0);
    let mut actions: Vec<TreeAction> = Vec::new();
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            let current = app.current_dir.clone();
            let shell = app.shell_icons.as_ref();
            render_node(ui, &mut app.tree_root, &current, &mut actions, shell);
        });

    for action in actions {
        match action {
            TreeAction::Toggle(path) => {
                if let Some(node) = app.tree_root.find_mut(&path) {
                    node.expanded = !node.expanded;
                    if node.expanded {
                        node.ensure_loaded();
                    }
                }
            }
            TreeAction::Select(path) => {
                if let Some(node) = app.tree_root.find_mut(&path) {
                    node.expanded = true;
                    node.ensure_loaded();
                }
                let ctx = ui.ctx().clone();
                app.open_dir(&ctx, path, None, Mode::Grid);
            }
        }
    }
}

fn render_node(
    ui: &mut egui::Ui,
    node: &mut TreeNode,
    current: &std::path::Path,
    actions: &mut Vec<TreeAction>,
    shell: Option<&crate::shell_icons::ShellIcons>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let arrow = if node.expanded { "▼" } else { "▶" };
        if ui.add(egui::Button::new(arrow).small().frame(false)).clicked() {
            actions.push(TreeAction::Toggle(node.path.clone()));
        }
        let selected = node.path.as_path() == current;
        let texture = shell.and_then(|icons| icons.texture_for(node.icon));
        let response = match texture {
            // Windows:资源管理器原生图标
            Some(texture) => {
                ui.add(egui::Image::new((texture.id(), egui::vec2(16.0, 16.0))));
                ui.selectable_label(selected, &node.name)
            }
            // 其他平台 / 提取失败:图标字形
            None => {
                let text = format!("{}  {}", node.icon.fallback_glyph(), node.name);
                ui.selectable_label(selected, text)
            }
        };
        if response.clicked() {
            actions.push(TreeAction::Select(node.path.clone()));
        }
        if response.double_clicked() {
            actions.push(TreeAction::Toggle(node.path.clone()));
        }
    });
    if node.expanded && !node.children.is_empty() {
        ui.indent(("tree", node.path.clone()), |ui| {
            for child in node.children.iter_mut() {
                render_node(ui, child, current, actions, shell);
            }
        });
    }
}

// ----------------------------------------------------------------------
// Thumbnail grid (center, Grid mode)
// ----------------------------------------------------------------------


const TILE_W: f32 = 168.0;
const TILE_H: f32 = 158.0;
const TILE_GAP: f32 = 10.0;
const IMG_H: f32 = 132.0;
const HEADER_YEAR_H: f32 = 38.0;
const HEADER_MONTH_H: f32 = 28.0;
const HEADER_DAY_H: f32 = 24.0;
const RAIL_W: f32 = 116.0;

enum GridNode {
    Folder(usize),
    Image(usize),
}

enum GridRowKind {
    Tiles(Vec<GridNode>),
    /// level: 0 = 年, 1 = 月, 2 = 日
    Header { level: u8, text: String },
}

struct GridRow {
    kind: GridRowKind,
    y: f32,
    height: f32,
}

/// 时间轴数据:年/月分组与各标题的内容坐标,供右侧导航条跳转。
struct GridLayout {
    rows: Vec<GridRow>,
    total_h: f32,
    groups: Vec<(i32, Vec<(u8, usize)>)>,
    year_y: Vec<(i32, f32)>,
    month_y: Vec<((i32, u8), f32)>,
    time_sorted: bool,
}

fn entry_time_parts(entry: &ImageEntry, key: SortKey) -> easyshow_core::DateTimeParts {
    match key {
        SortKey::Taken => entry.time_key(),
        SortKey::Modified => entry.modified_parts,
        _ => entry.created_parts,
    }
}

fn build_layout(app: &EasyShowApp, cols: usize) -> GridLayout {
    let mut rows: Vec<GridRow> = Vec::new();
    let mut y = 0.0f32;
    let mut groups: Vec<(i32, Vec<(u8, usize)>)> = Vec::new();
    let mut year_y: Vec<(i32, f32)> = Vec::new();
    let mut month_y: Vec<((i32, u8), f32)> = Vec::new();

    // 文件夹卡片始终排在最前。
    let mut i = 0;
    while i < app.folder_cards.len() {
        let end = (i + cols).min(app.folder_cards.len());
        rows.push(GridRow {
            kind: GridRowKind::Tiles((i..end).map(GridNode::Folder).collect()),
            y,
            height: TILE_H + TILE_GAP,
        });
        y += TILE_H + TILE_GAP;
        i = end;
    }

    let time_sorted = matches!(app.sort_key, SortKey::Taken | SortKey::Modified | SortKey::Created);
    let zh = app.lang == Lang::Zh;
    let mut current: Vec<GridNode> = Vec::new();
    let mut prev: Option<(i32, u8, u8)> = None;

    for (idx, entry) in app.entries.iter().enumerate() {
        if time_sorted {
            let parts = entry_time_parts(entry, app.sort_key);
            let key = (parts.year, parts.month, parts.day);
            if prev != Some(key) {
                if !current.is_empty() {
                    rows.push(GridRow {
                        kind: GridRowKind::Tiles(std::mem::take(&mut current)),
                        y,
                        height: TILE_H + TILE_GAP,
                    });
                    y += TILE_H + TILE_GAP;
                }
                let year_changed = prev.map(|p| p.0) != Some(key.0);
                let month_changed = year_changed || prev.map(|p| p.1) != Some(key.1);
                if year_changed {
                    year_y.push((key.0, y));
                    rows.push(GridRow {
                        kind: GridRowKind::Header {
                            level: 0,
                            text: if zh { format!("{} 年", key.0) } else { format!("{}", key.0) },
                        },
                        y,
                        height: HEADER_YEAR_H,
                    });
                    y += HEADER_YEAR_H;
                }
                if month_changed {
                    month_y.push(((key.0, key.1), y));
                    rows.push(GridRow {
                        kind: GridRowKind::Header {
                            level: 1,
                            text: if zh {
                                format!("{} 月", key.1)
                            } else {
                                format!("{}-{:02}", key.0, key.1)
                            },
                        },
                        y,
                        height: HEADER_MONTH_H,
                    });
                    y += HEADER_MONTH_H;
                    // 同一年内只保留一个分组,月份嵌套在年下面。
                    if groups.last().map(|(year, _)| *year) != Some(key.0) {
                        groups.push((key.0, Vec::new()));
                    }
                }
                rows.push(GridRow {
                    kind: GridRowKind::Header {
                        level: 2,
                        text: if zh {
                            format!("{} 月 {} 日", key.1, key.2)
                        } else {
                            format!("{}-{:02}-{:02}", key.0, key.1, key.2)
                        },
                    },
                    y,
                    height: HEADER_DAY_H,
                });
                y += HEADER_DAY_H;
                prev = Some(key);
            }
            if let Some((_, months)) = groups.last_mut() {
                match months.last_mut() {
                    Some((month, count)) if *month == key.1 => *count += 1,
                    _ => months.push((key.1, 1)),
                }
            }
        }
        current.push(GridNode::Image(idx));
        if current.len() == cols {
            rows.push(GridRow {
                kind: GridRowKind::Tiles(std::mem::take(&mut current)),
                y,
                height: TILE_H + TILE_GAP,
            });
            y += TILE_H + TILE_GAP;
        }
    }
    if !current.is_empty() {
        rows.push(GridRow {
            kind: GridRowKind::Tiles(current),
            y,
            height: TILE_H + TILE_GAP,
        });
        y += TILE_H + TILE_GAP;
    }

    GridLayout { rows, total_h: y + 12.0, groups, year_y, month_y, time_sorted }
}

pub fn grid(app: &mut EasyShowApp, ui: &mut egui::Ui) {
    let lang = app.lang;
    let palette = Palette::for_ui(ui);
    if app.entries.is_empty() && app.folder_cards.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(90.0);
            if app.folders_scanning {
                ui.spinner();
                ui.add_space(4.0);
                ui.weak(tr(lang, Key::FolderScanning));
            } else {
                ui.weak(tr(lang, Key::EmptyDir));
            }
        });
        return;
    }

    ui.add_space(8.0);
    let avail = ui.available_size();
    let time_sorted = matches!(app.sort_key, SortKey::Taken | SortKey::Modified | SortKey::Created);
    let rail_w = if time_sorted && !app.entries.is_empty() { RAIL_W } else { 0.0 };
    let grid_w = (avail.x - rail_w).max(160.0);
    let cols = ((((grid_w - 8.0) + TILE_GAP) / (TILE_W + TILE_GAP)).floor() as usize).max(1);
    app.grid_cols = cols;
    let layout = build_layout(app, cols);

    let mut select_idx: Option<(usize, bool, bool)> = None;
    let mut open_idx: Option<usize> = None;
    let mut select_folder: Option<usize> = None;
    let mut open_folder: Option<usize> = None;
    let mut to_request: Vec<std::path::PathBuf> = Vec::new();
    let mut to_upload: Vec<std::path::PathBuf> = Vec::new();
    let mut wanted: Vec<std::path::PathBuf> = Vec::new();
    let (ctrl, shift) = ui.input(|i| (i.modifiers.ctrl || i.modifiers.command, i.modifiers.shift));
    let mut scroll_offset = 0.0f32;

    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(grid_w, avail.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("grid-scroll")
                    .auto_shrink(false);
                if let Some(y) = app.grid_scroll_to.take() {
                    scroll = scroll.vertical_scroll_offset(y);
                }
                let output = scroll.show_viewport(ui, |ui, viewport| {
                    // 底部留一屏空白,保证最后一个月也能滚动到视口顶部。
                    ui.set_height(layout.total_h + (viewport.max.y - viewport.min.y));
                    let left = ui.max_rect().left();
                    let top = ui.max_rect().top();
                    let width = ui.max_rect().width();


                    // 视口上下各 2 行作为预载范围;缩略图线程会丢弃被甩出的排队项。
                    const PRELOAD_ROWS: usize = 2;
                    let pre_first = layout
                        .rows
                        .partition_point(|r| r.y + r.height < viewport.min.y)
                        .saturating_sub(PRELOAD_ROWS);
                    let pre_last = layout
                        .rows
                        .partition_point(|r| r.y <= viewport.max.y)
                        .saturating_add(PRELOAD_ROWS)
                        .min(layout.rows.len());
                    for row in &layout.rows[pre_first..pre_last] {
                        let GridRowKind::Tiles(nodes) = &row.kind else { continue };
                        for node in nodes {
                            match node {
                                GridNode::Image(idx) => {
                                    let path = app.entries[*idx].path.clone();
                                    wanted.push(path);
                                }
                                GridNode::Folder(fi) => {
                                    if let Some(cover) = app.folder_cards[*fi].cover.clone() {
                                        wanted.push(cover);
                                    }
                                }
                            }
                        }
                    }

                    let first = layout
                        .rows
                        .partition_point(|r| r.y + r.height < viewport.min.y);
                    let last = layout.rows.partition_point(|r| r.y <= viewport.max.y);
                    for row in &layout.rows[first..last] {
                        match &row.kind {
                            GridRowKind::Tiles(nodes) => {
                                for (col, node) in nodes.iter().enumerate() {
                                    let rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            left + col as f32 * (TILE_W + TILE_GAP),
                                            top + row.y,
                                        ),
                                        egui::vec2(TILE_W, TILE_H),
                                    );
                                    match node {
                                        GridNode::Image(idx) => {
                                            let primary =
                                                app.nav_order.get(app.selected).copied() == Some(*idx);
                                            let response = draw_photo_tile(
                                                ui,
                                                app,
                                                rect,
                                                *idx,
                                                primary,
                                                app.selection.contains(idx),
                                                &mut to_request,
                                                &mut to_upload,
                                            );
                                            if response.double_clicked() {
                                                open_idx = Some(*idx);
                                            } else if response.clicked() {
                                                select_idx = Some((*idx, ctrl, shift));
                                            }
                                        }
                                        GridNode::Folder(fi) => {
                                            let response = draw_folder_tile(
                                                ui,
                                                app,
                                                rect,
                                                *fi,
                                                app.selected_folder == Some(*fi),
                                                lang,
                                                &mut to_request,
                                                &mut to_upload,
                                            );
                                            if response.double_clicked() {
                                                open_folder = Some(*fi);
                                            } else if response.clicked() {
                                                select_folder = Some(*fi);
                                            }
                                        }
                                    }
                                }
                            }
                            GridRowKind::Header { level, text } => {
                                let color = match level {
                                    0 => ui.visuals().text_color(),
                                    1 => ui.visuals().text_color().gamma_multiply(0.9),
                                    _ => palette.text_dim,
                                };
                                let font = match level {
                                    0 => egui::FontId::proportional(15.0),
                                    1 => egui::FontId::proportional(13.0),
                                    _ => egui::FontId::proportional(11.5),
                                };
                                let rect = egui::Rect::from_min_size(
                                    egui::pos2(left, top + row.y),
                                    egui::vec2(width, row.height),
                                );
                                ui.painter().text(
                                    rect.left_center() + egui::vec2(10.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    text,
                                    font,
                                    color,
                                );
                                if *level < 2 {
                                    let line_y = rect.bottom() - 0.5;
                                    ui.painter().line_segment(
                                        [
                                            egui::pos2(rect.left() + 8.0, line_y),
                                            egui::pos2(rect.right() - 8.0, line_y),
                                        ],
                                        egui::Stroke::new(1.0, palette.text_dim.gamma_multiply(0.18)),
                                    );
                                }
                            }
                        }
                    }

                    // 吸顶年月标题:最近一条位于视口上方的月标题。
                    if layout.time_sorted {
                        let mut sticky_year: Option<&str> = None;
                        let mut sticky_month: Option<&str> = None;
                        for row in &layout.rows[..last] {
                            if row.y > viewport.min.y {
                                break;
                            }
                            if let GridRowKind::Header { level, text } = &row.kind {
                                match level {
                                    0 => {
                                        sticky_year = Some(text);
                                        sticky_month = None;
                                    }
                                    1 => sticky_month = Some(text),
                                    _ => {}
                                }
                            }
                        }
                        if let Some(year) = sticky_year {
                            let label = match sticky_month {
                                Some(month) => format!("{year}  ·  {month}"),
                                None => year.to_string(),
                            };
                            let bar = egui::Rect::from_min_size(
                                egui::pos2(left, top + viewport.min.y),
                                egui::vec2(width, 26.0),
                            );
                            ui.painter().rect_filled(bar, 0.0, palette.canvas_bg);
                            ui.painter().text(
                                bar.left_center() + egui::vec2(10.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                label,
                                egui::FontId::proportional(12.0),
                                palette.text_dim,
                            );
                            ui.painter().line_segment(
                                [bar.left_bottom(), bar.right_bottom()],
                                egui::Stroke::new(1.0, palette.text_dim.gamma_multiply(0.25)),
                            );
                        }
                    }
                });
                scroll_offset = output.state.clamped_offset().y;
            },
        );
        if rail_w > 0.0 {
            ui.allocate_ui_with_layout(
                egui::vec2(rail_w, avail.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| draw_timeline(app, ui, &layout, scroll_offset),
            );
        }
    });

    if open_folder.is_none() {
        app.set_visible_thumbs(wanted);
        if let Some(fi) = select_folder {
            app.selected_folder = Some(fi);
            app.selection.clear();
        }
        if let Some((idx, ctrl, shift)) = select_idx {
            app.select_with(idx, ctrl, shift);
        }
        if let Some(idx) = open_idx {
            app.select_with(idx, false, false);
            app.open_selected_in_viewer(ui.ctx());
        }
        for path in to_upload {
            if let Some(image) = app.thumb_imgs.get(&path) {
                let texture = upload_thumb_texture(ui.ctx(), image);
                app.thumb_tex.insert(path, texture);
            }
        }
        for path in to_request {
            app.request_thumb(&path);
        }
    }
    if let Some(fi) = open_folder {
        if let Some(card) = app.folder_cards.get(fi) {
            let dir = card.dir.clone();
            app.open_dir(ui.ctx(), dir, None, Mode::Grid);
        }
    }
}

fn draw_photo_tile(
    ui: &egui::Ui,
    app: &EasyShowApp,
    rect: egui::Rect,
    index: usize,
    primary: bool,
    selected: bool,
    to_request: &mut Vec<std::path::PathBuf>,
    to_upload: &mut Vec<std::path::PathBuf>,
) -> egui::Response {
    let palette = Palette::for_ui(ui);
    let (card_bg, thumb_bg, text_dim, accent) =
        (palette.card_bg, palette.thumb_bg, palette.text_dim, palette.accent);
    let painter = ui.painter();
    let entry = &app.entries[index];
    let path = &entry.path;

    painter.rect_filled(rect, egui::CornerRadius::same(8), card_bg);
    let img_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(4.0, 4.0),
        egui::vec2(TILE_W - 8.0, IMG_H),
    );
    painter.rect_filled(img_rect, egui::CornerRadius::same(6), thumb_bg);
    if let Some(texture) = app.thumb_tex.get(path) {
        draw_fitted(painter, texture.id(), img_rect, texture.size_vec2());
    } else if app.thumb_imgs.contains_key(path) {
        to_upload.push(path.clone());
    } else {
        to_request.push(path.clone());
    }

    let name = elide(&file_name(path), 20);
    painter.text(
        rect.center_bottom() - egui::vec2(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::proportional(11.5),
        text_dim,
    );
    if primary {
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(8),
            egui::Stroke::new(2.0, accent),
            egui::StrokeKind::Inside,
        );
    } else if selected {
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(8),
            egui::Stroke::new(1.5, accent.gamma_multiply(0.6)),
            egui::StrokeKind::Inside,
        );
    }

    let response = ui.interact(rect, ui.make_persistent_id(("grid-img", index)), egui::Sense::click());
    response.on_hover_text(file_name(path))
}

fn draw_folder_tile(
    ui: &egui::Ui,
    app: &EasyShowApp,
    rect: egui::Rect,
    folder_index: usize,
    selected: bool,
    lang: Lang,
    to_request: &mut Vec<std::path::PathBuf>,
    to_upload: &mut Vec<std::path::PathBuf>,
) -> egui::Response {
    let card = &app.folder_cards[folder_index];
    let palette = Palette::for_ui(ui);
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(8), palette.card_bg);
    let img_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(4.0, 4.0),
        egui::vec2(TILE_W - 8.0, IMG_H),
    );
    painter.rect_filled(img_rect, egui::CornerRadius::same(6), palette.thumb_bg);

    let mut has_cover = false;
    if let Some(cover) = &card.cover {
        if let Some(texture) = app.thumb_tex.get(cover) {
            draw_fitted(painter, texture.id(), img_rect, texture.size_vec2());
            has_cover = true;
        } else if app.thumb_imgs.contains_key(cover) {
            to_upload.push(cover.clone());
        } else {
            to_request.push(cover.clone());
        }
    }
    if !has_cover {
        // 封面尚未就绪时的占位:一个简洁的文件夹形状。
        let center = img_rect.center();
        let fw = 38.0;
        let fh = 28.0;
        let fg = palette.text_dim.gamma_multiply(0.4);
        let body = egui::Rect::from_min_size(
            egui::pos2(center.x - fw / 2.0, center.y - fh / 2.0 + 3.0),
            egui::vec2(fw, fh - 3.0),
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(body.left(), body.top() - 4.0),
                egui::vec2(fw * 0.45, 6.0),
            ),
            egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 },
            fg,
        );
        painter.rect_filled(body, egui::CornerRadius::same(3), fg);
    }

    // 左上角文件夹徽标
    let badge = egui::Rect::from_min_size(img_rect.min + egui::vec2(7.0, 7.0), egui::vec2(25.0, 22.0));
    painter.rect_filled(badge, egui::CornerRadius::same(5), egui::Color32::from_black_alpha(150));
    painter.text(
        badge.center(),
        egui::Align2::CENTER_CENTER,
        icons::FOLDER,
        egui::FontId::proportional(14.0),
        egui::Color32::WHITE,
    );

    // 右下角照片数量
    let count_text = card.photo_count.to_string();
    let galley = painter.layout_no_wrap(
        count_text,
        egui::FontId::proportional(10.5),
        egui::Color32::WHITE,
    );
    let pill = egui::Rect::from_min_size(
        egui::pos2(
            img_rect.right() - galley.size().x - 16.0,
            img_rect.bottom() - galley.size().y - 11.0,
        ),
        galley.size() + egui::vec2(10.0, 5.0),
    );
    painter.rect_filled(pill, egui::CornerRadius::same(4), egui::Color32::from_black_alpha(150));
    painter.galley(pill.min + egui::vec2(5.0, 2.5), galley, egui::Color32::WHITE);

    painter.text(
        rect.center_bottom() - egui::vec2(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        elide(&card.name, 20),
        egui::FontId::proportional(11.5),
        palette.text_dim,
    );
    if selected {
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(8),
            egui::Stroke::new(2.0, palette.accent),
            egui::StrokeKind::Inside,
        );
    }

    let response =
        ui.interact(rect, ui.make_persistent_id(("grid-folder", folder_index)), egui::Sense::click());
    let words = tr(lang, Key::FolderHint).replace("{n}", &card.photo_count.to_string());
    response.on_hover_text(words)
}

fn draw_timeline(app: &mut EasyShowApp, ui: &mut egui::Ui, layout: &GridLayout, offset: f32) {
    let lang = app.lang;
    let palette = Palette::for_ui(ui);
    let dim = palette.text_dim;
    let accent = palette.accent;

    // 当前年月:优先取视口顶部下方 140px 内的月标题(刚开始进入该月),
    // 否则取视口上方最近的月标题。
    let prev = layout
        .month_y
        .iter()
        .rev()
        .find(|(_, y)| *y <= offset + 1.0)
        .map(|(key, _)| *key);
    let next = layout
        .month_y
        .iter()
        .find(|(_, y)| *y >= offset - 1.0)
        .map(|(key, y)| (*key, *y));
    let active = match next {
        _ if prev.is_none() => next.map(|(key, _)| key),
        Some((key, y)) if y - offset < 140.0 => Some(key),
        _ => prev,
    };

    ui.painter().vline(
        ui.max_rect().left(),
        ui.max_rect().y_range(),
        egui::Stroke::new(1.0, dim.gamma_multiply(0.2)),
    );
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(egui::RichText::new(tr(lang, Key::Timeline)).size(11.0).color(dim));
    });
    ui.add_space(4.0);

    let mut clicked: Option<f32> = None;
    egui::ScrollArea::vertical()
        .id_salt("timeline-rail")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (year, months) in &layout.groups {
                let year_active = active.map(|(y, _)| y == *year).unwrap_or(false);
                let year_color = if year_active { accent } else { dim };
                let response = ui.add(
                    egui::Button::new(
                        egui::RichText::new(year.to_string())
                            .size(12.0)
                            .strong()
                            .color(year_color),
                    )
                    .frame(false)
                    .min_size(egui::vec2(100.0, 20.0)),
                );
                if response.clicked() {
                    clicked = layout
                        .year_y
                        .iter()
                        .find(|(y, _)| y == year)
                        .map(|(_, y)| *y);
                }
                for (month, count) in months {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let is_active = active == Some((*year, *month));
                        let color = if is_active { accent } else { dim };
                        let label = if lang == Lang::Zh {
                            format!("{month:02} 月")
                        } else {
                            format!("{month:02}")
                        };
                        let response = ui.add(
                            egui::Button::new(egui::RichText::new(label).size(11.0).color(color))
                                .frame(false),
                        );
                        if response.clicked() {
                            clicked = layout
                                .month_y
                                .iter()
                                .find(|(key, _)| key == &(*year, *month))
                                .map(|(_, y)| *y);
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.add_space(8.0);
                                ui.weak(egui::RichText::new(count.to_string()).size(9.5));
                            },
                        );
                    });
                }
            }
        });
    if let Some(y) = clicked {
        app.grid_scroll_to = Some(y);
    }
}

fn draw_fitted(painter: &egui::Painter, id: egui::TextureId, rect: egui::Rect, size: egui::Vec2) {
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let scale = (rect.width() / size.x).min(rect.height() / size.y);
    let draw_size = size * scale;
    let dest = egui::Rect::from_center_size(rect.center(), draw_size);
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(id, dest, uv, egui::Color32::WHITE);
}

fn elide(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

// ----------------------------------------------------------------------
// Viewer canvas (center, Viewer mode)
// ----------------------------------------------------------------------

pub fn canvas(app: &mut EasyShowApp, ui: &mut egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    app.canvas_size = rect.size();

    let response = ui
        .allocate_rect(rect, egui::Sense::click_and_drag())
        .on_hover_and_drag_cursor(egui::CursorIcon::Grab);
    ui.painter().rect_filled(rect, 0.0, Palette::for_ui(ui).canvas_bg);

    if app.current.is_none() {
        let msg = if app.loading { tr(app.lang, Key::Loading) } else { tr(app.lang, Key::EmptyHint) };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            msg,
            egui::FontId::proportional(15.0),
            egui::Color32::from_gray(150),
        );
        return;
    }

    let size = app.current.as_ref().map(display_size).unwrap();
    if app.viewer.mode == ZoomMode::Fit {
        app.viewer.recompute_fit(size, rect.size());
    }

    let center = rect.center();
    let image_rect = app.viewer.image_rect(size, center);
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    let texture = app.current.as_ref().unwrap().texture.id();
    ui.painter()
        .with_clip_rect(rect)
        .image(texture, image_rect, uv, egui::Color32::WHITE);

    // 左键拖动平移
    if response.dragged() {
        app.viewer.offset += response.drag_delta();
    }
    // 中键拖动平移
    let middle_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle));
    if middle_down && response.hovered() {
        app.viewer.offset += ui.input(|i| i.pointer.delta());
    }
    if response.double_clicked() {
        if app.viewer.mode == ZoomMode::Fit {
            app.set_actual();
        } else {
            app.set_fit();
        }
    }

    // 触控板捏合缩放
    let pinch = ui.input(|i| i.zoom_delta());
    if (pinch - 1.0).abs() > 0.002 {
        let anchor = ui.input(|i| i.pointer.hover_pos()).unwrap_or(center);
        let new_zoom = (app.viewer.zoom * pinch).clamp(0.05, 8.0);
        app.viewer.zoom_at(new_zoom, anchor, center);
        app.viewer.mode = ZoomMode::Fixed(new_zoom);
        app.maybe_request_full_res();
    }

    // 滚轮:按"格"计量(应用层已归一化),默认缩放;设置里可改为翻页(Ctrl 仍缩放)
    let wheel_units = std::mem::take(&mut app.pending_wheel_units);
    if response.hovered() && wheel_units != 0.0 {
        let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        if !app.wheel_navigates || ctrl {
            let anchor = ui.input(|i| i.pointer.hover_pos()).unwrap_or(center);
            app.zoom_wheel(wheel_units, anchor, center);
        } else {
            app.navigate_wheel(ui.ctx(), wheel_units);
        }
    }
}

// ----------------------------------------------------------------------
// Info panel (right)
// ----------------------------------------------------------------------

pub fn info_panel(app: &mut EasyShowApp, ui: &mut egui::Ui) {
    let lang = app.lang;
    ui.add_space(8.0);
    ui.label(egui::RichText::new(tr(lang, Key::Info)).strong());
    ui.add_space(2.0);
    if let Some(card) = app.selected_folder.and_then(|i| app.folder_cards.get(i)) {
        ui.add(egui::Label::new(&card.name).truncate());
        ui.add_space(6.0);
        ui.weak(format!("{} {}", card.photo_count, tr(lang, Key::Photos)));
        let when = easyshow_core::timeutil::from_system_time_local(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(card.latest_mtime),
        );
        kv(
            ui,
            tr(lang, Key::Modified),
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                when.year, when.month, when.day, when.hour, when.minute
            ),
        );
        ui.add_space(8.0);
        ui.weak(tr(lang, Key::DoubleClickEnter));
        return;
    }
    let Some(info) = &app.info else {
        ui.weak("—");
        return;
    };

    ui.add(egui::Label::new(file_name(&info.path)).truncate());
    ui.add_space(6.0);

    let dash = || "—".to_string();
    kv(ui, tr(lang, Key::Dimensions), info.dims.map(|(w, h)| format!("{w} × {h}")).unwrap_or_else(dash));
    kv(ui, tr(lang, Key::Size), info.file_size.map(human_size).unwrap_or_else(dash));
    kv(ui, tr(lang, Key::Format), info.format.map(str::to_string).unwrap_or_else(dash));
    kv(ui, tr(lang, Key::Modified), info.modified.clone().unwrap_or_else(dash));

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(egui::RichText::new(tr(lang, Key::Exif)).strong());
    ui.add_space(4.0);

    match &info.exif {
        Some(exif) => {
            let camera = match (&exif.make, &exif.model) {
                (Some(make), Some(model)) => Some(format!("{make} {model}")),
                (Some(make), None) => Some(make.clone()),
                (None, Some(model)) => Some(model.clone()),
                (None, None) => None,
            };
            if let Some(camera) = camera {
                kv(ui, tr(lang, Key::Camera), camera);
            }
            if let Some(lens) = &exif.lens {
                kv(ui, tr(lang, Key::Lens), lens.clone());
            }
            if let Some(f) = &exif.f_number {
                kv(ui, tr(lang, Key::Aperture), f.clone());
            }
            if let Some(exposure) = &exif.exposure {
                kv(ui, tr(lang, Key::Shutter), exposure.clone());
            }
            if let Some(iso) = exif.iso {
                kv(ui, tr(lang, Key::Iso), iso.to_string());
            }
            if let Some(focal) = &exif.focal_length {
                kv(ui, tr(lang, Key::FocalLength), focal.clone());
            }
            if let Some(taken) = &exif.date_time {
                kv(ui, tr(lang, Key::Taken), taken.clone());
            }
            if let Some((lat, lon)) = exif.gps {
                let coords = format!("{lat:.6}, {lon:.6}");
                let place_label = app.place.as_ref().and_then(|p| p.label());
                let value = if let Some(label) = &place_label {
                    label.clone()
                } else if app.place_loading {
                    tr(lang, Key::Locating).to_string()
                } else {
                    coords.clone()
                };
                kv(ui, tr(lang, Key::Location), value);
                if place_label.is_some() {
                    kv(ui, tr(lang, Key::Coordinates), coords);
                }
            }
        }
        None => {
            ui.weak(tr(lang, Key::NoExif));
        }
    }
}

fn kv(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.weak(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // 单行省略号截断;egui 对"被截断"的标签自动悬停显示全文,无需手动 tooltip
            ui.add(egui::Label::new(value).truncate());
        });
    });
}

// ----------------------------------------------------------------------
// Status bar (bottom)
// ----------------------------------------------------------------------

pub fn status(app: &mut EasyShowApp, ui: &mut egui::Ui) {
    let lang = app.lang;
    ui.horizontal(|ui| {
        if let Some(card) = app.selected_folder.and_then(|i| app.folder_cards.get(i)) {
            ui.label(&card.name);
            ui.weak(format!("{} {}", card.photo_count, tr(lang, Key::Photos)));
            ui.weak(tr(lang, Key::DoubleClickEnter));
        } else if let Some(info) = &app.info {
            ui.label(file_name(&info.path));
            ui.weak(format!("{} / {}", app.selected + 1, app.nav_order.len()));
            if app.selection.len() > 1 {
                ui.weak(tr(lang, Key::SelectedCount).replace("{n}", &app.selection.len().to_string()));
            }
            if let Some((w, h)) = info.dims {
                ui.weak(format!("{w} × {h}"));
            }
            if let Some(fmt) = info.format {
                ui.weak(fmt);
            }
            if let Some(size) = info.file_size {
                ui.weak(human_size(size));
            }
            if let Some(modified) = &info.modified {
                ui.weak(modified.clone());
            }
        } else {
            ui.label(tr(lang, Key::EmptyHint));
        }
        if let Some(err) = &app.error {
            ui.colored_label(egui::Color32::from_rgb(0xff, 0x76, 0x6e), err);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.loading {
                ui.weak(tr(lang, Key::Loading));
            } else if app.mode == Mode::Viewer {
                ui.weak(app.zoom_label());
            } else if app.mode == Mode::Map {
                let with_location = app
                    .entries
                    .iter()
                    .filter(|e| app.geo_points.get(&e.path).copied().flatten().is_some())
                    .count();
                ui.weak(
                    tr(lang, Key::MapPhotos)
                        .replace("{n}", &with_location.to_string())
                        .replace("{m}", &app.entries.len().to_string()),
                );
                ui.weak(format!("z {:.1}", app.map.zoom));
            } else {
                ui.weak(tr(lang, Key::GridHint));
            }
            #[cfg(feature = "formats")]
            if app.anim_active() {
                ui.weak(format!("● {}", tr(lang, Key::Animated)));
            }
            if app.time_scanned < app.time_total {
                ui.weak(format!(
                    "{} {}/{}",
                    tr(lang, Key::TimeScanning),
                    app.time_scanned,
                    app.time_total
                ));
            }
            if app.is_slideshow() {
                ui.weak(format!("▶ {}", tr(lang, Key::Slideshow)));
            }
        });
    });
}

// ----------------------------------------------------------------------
// 删除确认(模态)
// ----------------------------------------------------------------------

pub fn delete_modal(app: &mut EasyShowApp, ctx: &egui::Context) {
    let Some(paths) = app.pending_delete.clone() else { return };
    let lang = app.lang;
    let mut cancel = false;
    let mut confirm = false;
    egui::Window::new(tr(lang, Key::DeleteConfirmTitle))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.label(tr(lang, Key::DeleteConfirmBody).replace("{n}", &paths.len().to_string()));
            ui.add_space(6.0);
            for path in paths.iter().take(5) {
                ui.weak(file_name(path));
            }
            if paths.len() > 5 {
                ui.weak(format!("… +{}", paths.len() - 5));
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr(lang, Key::Cancel)).clicked() {
                    cancel = true;
                }
                let destructive = egui::Button::new(
                    egui::RichText::new(tr(lang, Key::ConfirmDelete)).color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgb(0xc9, 0x45, 0x3f));
                if ui.add(destructive).clicked() {
                    confirm = true;
                }
            });
        });
    if cancel {
        app.cancel_delete();
    } else if confirm {
        app.confirm_delete(ctx);
    }
}
