//! Toolbar (Apple 风格图标), directory tree, thumbnail grid, viewer canvas,
//! info panel and status bar.

use eframe::egui;

use crate::app::{display_size, file_name, human_size, upload_thumb_texture, LiteshowApp, Mode};
use crate::i18n::{tr, Key, Lang};
use crate::icons;
use crate::sort::SortKey;
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
        SortKey::Modified => tr(lang, Key::ByModified),
        SortKey::Created => tr(lang, Key::ByCreated),
        SortKey::Size => tr(lang, Key::BySize),
        SortKey::Type => tr(lang, Key::ByType),
    }
}

// ----------------------------------------------------------------------
// Toolbar
// ----------------------------------------------------------------------

pub fn toolbar(app: &mut LiteshowApp, ui: &mut egui::Ui) {
    let lang = app.lang;
    let mut pick_sort: Option<SortKey> = None;
    let mut flip_sort_dir = false;
    let mut pick_lang: Option<Lang> = None;
    let mut pick_theme: Option<ThemeMode> = None;

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
                ui.weak(format!("Liteshow v{}", env!("CARGO_PKG_VERSION")));
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
                for key in [SortKey::Name, SortKey::Modified, SortKey::Created, SortKey::Size, SortKey::Type] {
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
        });
    });

    if let Some(key) = pick_sort {
        app.set_sort(key);
    }
    if flip_sort_dir {
        app.toggle_sort_direction();
    }
    if let Some(lang) = pick_lang {
        app.lang = lang;
        app.relabel_tree_root();
    }
    if let Some(mode) = pick_theme {
        app.set_theme_mode(ui.ctx(), mode);
    }
}

// ----------------------------------------------------------------------
// Directory tree (left)
// ----------------------------------------------------------------------

enum TreeAction {
    Toggle(std::path::PathBuf),
    Select(std::path::PathBuf),
}

pub fn tree_panel(app: &mut LiteshowApp, ui: &mut egui::Ui) {
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

pub fn grid(app: &mut LiteshowApp, ui: &mut egui::Ui) {
    let lang = app.lang;
    ui.add_space(8.0);
    if app.entries.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(90.0);
            ui.weak(tr(lang, Key::EmptyDir));
        });
        return;
    }

    let avail = ui.available_width();
    let cols = (((avail - 8.0 + TILE_GAP) / (TILE_W + TILE_GAP)).floor() as usize).max(1);
    app.grid_cols = cols;
    let rows = app.entries.len().div_ceil(cols);
    let row_h = TILE_H + TILE_GAP;

    let mut select_idx: Option<usize> = None;
    let mut open_idx: Option<usize> = None;
    let mut to_request: Vec<std::path::PathBuf> = Vec::new();
    let mut to_upload: Vec<std::path::PathBuf> = Vec::new();
    let palette = Palette::for_ui(ui);
    let (card_bg, thumb_bg, text_dim, accent) = (palette.card_bg, palette.thumb_bg, palette.text_dim, palette.accent);

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show_rows(ui, row_h, rows, |ui, row_range| {
            ui.spacing_mut().item_spacing = egui::vec2(TILE_GAP, TILE_GAP);
            for row in row_range {
                ui.horizontal(|ui| {
                    for col in 0..cols {
                        let idx = row * cols + col;
                        let Some(entry) = app.entries.get(idx) else { break };
                        let path = entry.path.clone();
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(TILE_W, TILE_H), egui::Sense::click());
                        let painter = ui.painter().clone();

                        painter.rect_filled(rect, egui::CornerRadius::same(8), card_bg);
                        let img_rect = egui::Rect::from_min_size(
                            rect.min + egui::vec2(4.0, 4.0),
                            egui::vec2(TILE_W - 8.0, IMG_H),
                        );
                        painter.rect_filled(img_rect, egui::CornerRadius::same(6), thumb_bg);
                        if let Some(texture) = app.thumb_tex.get(&path) {
                            draw_fitted(&painter, texture.id(), img_rect, texture.size_vec2());
                        } else if app.thumb_imgs.contains_key(&path) {
                            to_upload.push(path.clone());
                        } else {
                            to_request.push(path.clone());
                        }

                        let name = elide(&file_name(&path), 20);
                        painter.text(
                            rect.center_bottom() - egui::vec2(0.0, 12.0),
                            egui::Align2::CENTER_CENTER,
                            name,
                            egui::FontId::proportional(11.5),
                            text_dim,
                        );
                        if idx == app.selected {
                            painter.rect_stroke(
                                rect,
                                egui::CornerRadius::same(8),
                                egui::Stroke::new(2.0, accent),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if response.double_clicked() {
                            open_idx = Some(idx);
                        } else if response.clicked() {
                            select_idx = Some(idx);
                        }
                    }
                });
            }
        });

    if let Some(idx) = select_idx {
        app.select(idx);
    }
    if let Some(idx) = open_idx {
        app.select(idx);
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

pub fn canvas(app: &mut LiteshowApp, ui: &mut egui::Ui) {
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

pub fn info_panel(app: &mut LiteshowApp, ui: &mut egui::Ui) {
    let lang = app.lang;
    ui.add_space(8.0);
    ui.label(egui::RichText::new(tr(lang, Key::Info)).strong());
    ui.add_space(2.0);
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
                kv(ui, tr(lang, Key::Location), format!("{lat:.6}, {lon:.6}"));
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

pub fn status(app: &mut LiteshowApp, ui: &mut egui::Ui) {
    let lang = app.lang;
    ui.horizontal(|ui| {
        if let Some(info) = &app.info {
            ui.label(file_name(&info.path));
            ui.weak(format!("{} / {}", app.selected + 1, app.entries.len()));
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
            } else {
                ui.weak(tr(lang, Key::GridHint));
            }
        });
    });
}
