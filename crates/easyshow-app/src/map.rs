//! 地图模式:离线世界底图 + 在线瓦片 + 照片标记/聚合。

use std::collections::HashSet;

use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::{file_name, EasyShowApp, Mode};
use crate::geo::{self, ScreenCluster};
use crate::i18n::{tr, Key};
use crate::theme::Palette;
use crate::tiles::{TileKey, TileKind};

/// 聚合阈值(屏幕像素)。
const CLUSTER_CELL: f32 = 44.0;
/// 离线世界图的最大缩放(再放大就只有国界轮廓,没有意义)。
pub const MAX_ZOOM_OFFLINE: f64 = 4.2;
/// 在线瓦片的最大缩放。
pub const MAX_ZOOM_ONLINE: f64 = 17.0;

struct MapColors {
    water: Color32,
    land: Color32,
    border: Color32,
    cluster_text: Color32,
}

fn colors(dark: bool) -> MapColors {
    if dark {
        MapColors {
            water: Color32::from_rgb(0x0a, 0x0f, 0x17),
            land: Color32::from_rgb(0x1c, 0x24, 0x31),
            border: Color32::from_rgb(0x37, 0x45, 0x5a),
            cluster_text: Color32::from_rgb(0xff, 0xff, 0xff),
        }
    } else {
        MapColors {
            water: Color32::from_rgb(0xd7, 0xe3, 0xef),
            land: Color32::from_rgb(0xfb, 0xfd, 0xff),
            border: Color32::from_rgb(0xb2, 0xc3, 0xd4),
            cluster_text: Color32::from_rgb(0x0b, 0x1b, 0x2a),
        }
    }
}

/// 世界蒙版纹理(首次进入地图时解码上传)。
fn ensure_world_textures(app: &mut EasyShowApp, ctx: &egui::Context) {
    if app.world_map_loaded {
        return;
    }
    app.world_map_loaded = true;
    let upload = |ctx: &egui::Context, name: &str, bytes: &[u8]| -> Option<egui::TextureHandle> {
        let image = image::load_from_memory(bytes).ok()?.to_rgba8();
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        );
        Some(ctx.load_texture(name, color, egui::TextureOptions::LINEAR))
    };
    app.world_land = upload(ctx, "world-land", include_bytes!("../../../assets/world-land.png"));
    app.world_borders = upload(ctx, "world-borders", include_bytes!("../../../assets/world-borders.png"));
}

fn draw_world_map(app: &EasyShowApp, painter: &egui::Painter, rect: Rect, colors: &MapColors) {
    let (Some(land), Some(borders)) = (&app.world_land, &app.world_borders) else { return };
    let size = (rect.width() as f64, rect.height() as f64);
    let a = app.map.world_to_screen(size, (0.0, 0.0));
    let b = app.map.world_to_screen(size, (1.0, 1.0));
    let dest = Rect::from_min_max(
        egui::pos2(rect.min.x + a.0 as f32, rect.min.y + a.1 as f32),
        egui::pos2(rect.min.x + b.0 as f32, rect.min.y + b.1 as f32),
    );
    painter.image(land.id(), dest, Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)), colors.land);
    painter.image(borders.id(), dest, Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)), colors.border);
}

/// 当前应使用的瓦片源(联网关闭时为 None)。
pub fn tile_kind(app: &EasyShowApp) -> Option<TileKind> {
    if !app.net.online {
        return None;
    }
    let chinese = app.lang == crate::i18n::Lang::Zh;
    TileKind::from_source(app.net.resolved_tile_source(chinese))
}

/// 绘制在线瓦片,返回 (可见瓦片集, 待请求瓦片)。调用方需先 `set_wanted`
/// 再发请求——工作线程会丢弃不在可见集里的任务。
fn draw_tiles(
    app: &mut EasyShowApp,
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: Rect,
    kind: TileKind,
    colors: &MapColors,
) -> (HashSet<TileKey>, Vec<TileKey>) {
    let size = (rect.width() as f64, rect.height() as f64);
    let z = app.map.zoom.round().clamp(0.0, MAX_ZOOM_ONLINE) as u8;
    let n = 2f64.powi(z as i32);
    let w0 = app.map.screen_to_world(size, (0.0, 0.0));
    let w1 = app.map.screen_to_world(size, (size.0, size.1));
    let max_ix = n as i64 - 1;
    let x0 = ((w0.0 * n).floor() as i64).clamp(0, max_ix);
    let y0 = ((w0.1 * n).floor() as i64).clamp(0, max_ix);
    let x1 = ((w1.0 * n).ceil() as i64).clamp(0, max_ix);
    let y1 = ((w1.1 * n).ceil() as i64).clamp(0, max_ix);

    let mut wanted = HashSet::new();
    let mut to_request: Vec<TileKey> = Vec::new();
    // 预取一圈,拖动时不闪白
    for y in (y0 - 1).max(0)..=(y1 + 1).min(max_ix) {
        for x in (x0 - 1).max(0)..=(x1 + 1).min(max_ix) {
            let key = TileKey { z, x: x as u32, y: y as u32 };
            wanted.insert(key);
            let tk = (kind, key);
            if let Some(texture) = app.tile_tex.get(&tk) {
                let wa = (x as f64 / n, y as f64 / n);
                let wb = ((x + 1) as f64 / n, ((y + 1) as f64 / n));
                let sa = app.map.world_to_screen(size, wa);
                let sb = app.map.world_to_screen(size, wb);
                let dest = Rect::from_min_max(
                    egui::pos2(rect.min.x + sa.0 as f32, rect.min.y + sa.1 as f32),
                    egui::pos2(rect.min.x + sb.0 as f32, rect.min.y + sb.1 as f32),
                );
                painter.image(
                    texture.id(),
                    dest,
                    Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else if !app.tile_inflight.contains(&tk) && !app.tile_failed.contains(&tk) {
                to_request.push(key);
            }
        }
    }
    if !app.tile_inflight.is_empty() {
        ui.ctx().request_repaint();
    }

    // 底图版权标注
    let attribution = match kind {
        TileKind::Amap => "© 高德地图",
        TileKind::Osm => "© OpenStreetMap contributors",
    };
    painter.text(
        egui::pos2(rect.right() - 8.0, rect.bottom() - 6.0),
        Align2::RIGHT_BOTTOM,
        attribution,
        FontId::proportional(10.0),
        colors.border,
    );
    (wanted, to_request)
}

fn collect_clusters(app: &EasyShowApp, rect: Rect, uses_gcj: bool) -> Vec<ScreenCluster> {
    let size = (rect.width() as f64, rect.height() as f64);
    let mut points: Vec<(usize, f32, f32)> = Vec::new();
    for (idx, entry) in app.entries.iter().enumerate() {
        let Some(Some(latlon)) = app.geo_points.get(&entry.path).copied() else { continue };
        let latlon = if uses_gcj { geo::wgs84_to_gcj02(latlon) } else { latlon };
        let world = geo::lonlat_to_world(latlon);
        let (x, y) = app.map.world_to_screen(size, world);
        if x < -40.0 || y < -40.0 || x > size.0 + 40.0 || y > size.1 + 40.0 {
            continue;
        }
        points.push((idx, x as f32, y as f32));
    }
    geo::cluster_points(&points, CLUSTER_CELL)
}

fn cluster_radius(members: usize) -> f32 {
    if members <= 1 {
        5.0
    } else {
        9.0 + (members as f32).ln() * 3.5
    }
}

fn draw_clusters(
    app: &EasyShowApp,
    painter: &egui::Painter,
    rect: Rect,
    clusters: &[ScreenCluster],
    palette: &Palette,
    colors: &MapColors,
) {
    let selected = app.entry_index();
    for cluster in clusters {
        let center = egui::pos2(rect.min.x + cluster.x, rect.min.y + cluster.y);
        if cluster.members.len() == 1 {
            let idx = cluster.members[0];
            let is_selected = selected == Some(idx);
            painter.circle_filled(center, 4.0, palette.accent);
            painter.circle_stroke(center, 4.8, Stroke::new(1.0, Color32::from_white_alpha(180)));
            if is_selected {
                painter.circle_stroke(center, 8.5, Stroke::new(1.6, palette.accent));
            }
        } else {
            let r = cluster_radius(cluster.members.len());
            painter.circle_filled(center, r + 1.5, Color32::from_black_alpha(35));
            painter.circle_filled(center, r, palette.accent.gamma_multiply(0.30));
            painter.circle_stroke(center, r, Stroke::new(1.2, palette.accent));
            painter.text(
                center,
                Align2::CENTER_CENTER,
                cluster.members.len().to_string(),
                FontId::proportional(12.0),
                colors.cluster_text,
            );
        }
    }
}

/// 交互:拖动平移、滚轮缩放、点击标记/聚合、双击进浏览。
fn interact(
    app: &mut EasyShowApp,
    ui: &egui::Ui,
    response: &egui::Response,
    rect: Rect,
    clusters: &[ScreenCluster],
) {
    let size = (rect.width() as f64, rect.height() as f64);

    if response.dragged() {
        let delta = ui.input(|i| i.pointer.delta());
        app.map.pan_px(delta.x as f64, delta.y as f64);
    }

    if ui.rect_contains_pointer(rect) {
        let units = std::mem::take(&mut app.pending_wheel_units);
        if units != 0.0 {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                let rel = ((pos.x - rect.min.x) as f64, (pos.y - rect.min.y) as f64);
                app.map.zoom_by_steps(rel, size, units, 0.0, app.max_map_zoom());
            }
            // 缩放后瓦片层级变化,清理失败集以便重试
            if units.abs() > 0.5 {
                app.tile_failed.clear();
            }
        }
    }

    // 标记/聚合的点击与悬停
    for (ci, cluster) in clusters.iter().enumerate() {
        let r = cluster_radius(cluster.members.len()) + 3.0;
        let center = egui::pos2(rect.min.x + cluster.x, rect.min.y + cluster.y);
        let hit = ui.interact(
            Rect::from_center_size(center, Vec2::splat(r * 2.0)),
            ui.scope_id().with(("map-cluster", ci)),
            Sense::click(),
        );
        if ci == 0 {
            log::trace!("map: rect={:?}", rect);
        }
        if hit.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if hit.clicked() {
            log::debug!("map: cluster clicked members={}", cluster.members.len());
            if cluster.members.len() > 1 {
                // 放大以展开聚合
                let world = app.map.screen_to_world(size, (cluster.x as f64, cluster.y as f64));
                app.map.center = geo::world_to_lonlat(world);
                app.map.zoom = (app.map.zoom + 1.5).min(app.max_map_zoom());
                app.tile_failed.clear();
            } else {
                let idx = cluster.members[0];
                app.select_with(idx, false, false);
                if hit.double_clicked() {
                    app.mode = Mode::Viewer;
                    app.load_current(ui.ctx());
                }
            }
        }
        if cluster.members.len() == 1 {
            if let Some(entry) = app.entries.get(cluster.members[0]) {
                hit.on_hover_text(file_name(&entry.path));
            }
        } else {
            let mut names: Vec<String> = cluster
                .members
                .iter()
                .take(4)
                .filter_map(|&i| app.entries.get(i))
                .map(|e| file_name(&e.path))
                .collect();
            if cluster.members.len() > 4 {
                names.push("…".to_string());
            }
            hit.on_hover_text(format!("{}\n{}", cluster.members.len(), names.join("\n")));
        }
    }
}

/// 地图视图入口(由 CentralPanel 调用)。
pub fn map_view(app: &mut EasyShowApp, ui: &mut egui::Ui) {
    let lang = app.lang;
    let palette = Palette::for_ui(ui);
    let map_colors = colors(ui.visuals().dark_mode);

    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = response.rect;
    app.map_area = rect.size();
    painter.rect_filled(rect, 0.0, map_colors.water);

    ensure_world_textures(app, ui.ctx());
    draw_world_map(app, &painter, rect, &map_colors);

    let kind = tile_kind(app);
    match kind {
        Some(kind) => {
            let (wanted, to_request) = draw_tiles(app, ui, &painter, rect, kind, &map_colors);
            // 先登记可见集,再发请求(工作线程按可见集过滤)
            app.tiles.set_wanted(kind, &wanted);
            for key in to_request {
                let tk = (kind, key);
                app.tile_inflight.insert(tk);
                app.tiles.request(kind, key, app.tile_generation);
            }
        }
        None => {
            app.tiles.clear_wanted();
        }
    }

    let uses_gcj = kind == Some(TileKind::Amap);
    let clusters = collect_clusters(app, rect, uses_gcj);
    draw_clusters(app, &painter, rect, &clusters, palette, &map_colors);

    // 扫描进度 / 空态 / 离线提示
    let scanned = app.geo_scanned;
    let total = app.geo_total;
    if total > 0 && scanned < total {
        painter.text(
            egui::pos2(rect.left() + 12.0, rect.top() + 10.0),
            Align2::LEFT_TOP,
            format!("{} {scanned}/{total}", tr(lang, Key::GeoScanning)),
            FontId::proportional(12.5),
            palette.text_dim,
        );
        ui.ctx().request_repaint();
    } else {
        let with_location = clusters.iter().map(|c| c.members.len()).sum::<usize>();
        if total > 0 && with_location == 0 {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                tr(lang, Key::NoGeoPhotos),
                FontId::proportional(14.0),
                palette.text_dim,
            );
        }
    }
    if kind.is_none() {
        painter.text(
            egui::pos2(rect.right() - 8.0, rect.bottom() - 6.0),
            Align2::RIGHT_BOTTOM,
            tr(lang, Key::MapOfflineHint),
            FontId::proportional(10.5),
            palette.text_dim,
        );
    }

    interact(app, ui, &response, rect, &clusters);
}
