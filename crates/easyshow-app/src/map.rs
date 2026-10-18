//! 地图模式:离线世界底图 + 在线瓦片 + 照片缩略图聚合。
//!
//! 聚合点直接显示照片微缩图(随缩放级别展示 1 / 4 / 9 张),点开弹层可浏览
//! 该聚合下的全部照片(按缩略图网格),单击选中、双击进入浏览。

use std::collections::HashSet;
use std::path::PathBuf;

use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::app::{file_name, EasyShowApp, Mode};
use crate::geo::{self, ScreenCluster};
use crate::i18n::{tr, Key};
use crate::theme::Palette;
use crate::tiles::{TileKey, TileKind};

/// 聚合阈值(屏幕像素)。
const CLUSTER_CELL: f32 = 44.0;
/// 离线世界图的最大缩放(再放大就只有国界轮廓,没有意义)。
pub const MAX_ZOOM_OFFLINE: f64 = 6.0;
/// 在线瓦片的最大缩放。
pub const MAX_ZOOM_ONLINE: f64 = 17.0;
/// 聚合弹层最多展示的照片数。
const POPUP_LIMIT: usize = 12;

/// 聚合点弹出的照片网格。
#[derive(Clone)]
pub struct MapPopup {
    pub members: Vec<usize>,
    pub anchor: Pos2,
    pub opened_at: f64,
}

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

/// 聚合点随缩放展示的缩略图数量与网格布局。
/// 低缩放每簇 1 张;放大后依次展示 4 张、9 张,同时瓦片也会随着缩放逐渐分开。
fn cluster_layout(zoom: f64, count: usize) -> (usize, f32, Vec<(f32, f32)>) {
    let max_tiles = if zoom < 3.0 {
        1
    } else if zoom < 4.6 {
        4
    } else {
        9
    };
    let shown = count.min(max_tiles).max(1);
    let tile_r = (10.0f32 + ((zoom - 3.0).clamp(0.0, 8.0) as f32) * 1.3).clamp(10.0, 20.0);
    let gap = tile_r * 1.8;
    let cols = match shown {
        0 | 1 => 1,
        2..=4 => 2,
        _ => 3,
    };
    let rows = (shown + cols - 1) / cols;
    let mut positions = Vec::with_capacity(shown);
    for i in 0..shown {
        let row = i / cols;
        let col = i % cols;
        let in_row = if row + 1 == rows { shown - row * cols } else { cols };
        let x = (col as f32 - (in_row as f32 - 1.0) / 2.0) * gap;
        let y = (row as f32 - (rows as f32 - 1.0) / 2.0) * gap;
        positions.push((x, y));
    }
    (shown, tile_r, positions)
}

/// 聚合点(缩略图卡片)的交互矩形。
fn cluster_bounds(zoom: f64, count: usize, center: Pos2) -> Rect {
    let (_, tile_r, positions) = cluster_layout(zoom, count);
    let mut bounds = Rect::from_center_size(center, Vec2::splat(tile_r * 2.0));
    for (x, y) in &positions {
        let c = center + Vec2::new(*x, *y);
        bounds = bounds.union(Rect::from_center_size(c, Vec2::splat(tile_r * 2.0)));
    }
    bounds.expand(4.0)
}

/// 把纹理会成圆形(三角扇 + 圆形 UV 裁剪,纹理按中心正方形裁剪)。
fn circular_image(
    painter: &egui::Painter,
    texture: &egui::TextureHandle,
    center: Pos2,
    radius: f32,
    border: Stroke,
) {
    let size = texture.size_vec2();
    let side = size.x.min(size.y).max(1.0);
    let half_u = 0.5 * side / size.x.max(1.0);
    let half_v = 0.5 * side / size.y.max(1.0);

    let mut mesh = egui::epaint::Mesh::with_texture(texture.id());
    let center_idx = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: center,
        uv: egui::pos2(0.5, 0.5),
        color: Color32::WHITE,
    });
    const SEGMENTS: usize = 28;
    for i in 0..SEGMENTS {
        let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        mesh.vertices.push(egui::epaint::Vertex {
            pos: center + Vec2::new(cos, sin) * radius,
            uv: egui::pos2(0.5 + cos * half_u, 0.5 + sin * half_v),
            color: Color32::WHITE,
        });
    }
    for i in 0..SEGMENTS as u32 {
        let current = center_idx + 1 + i;
        let next = center_idx + 1 + (i + 1) % SEGMENTS as u32;
        mesh.indices.extend_from_slice(&[center_idx, current, next]);
    }
    painter.add(egui::Shape::mesh(mesh));
    painter.circle_stroke(center, radius, border);
}

fn draw_photo_tile(
    app: &EasyShowApp,
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    idx: usize,
    palette: &Palette,
    colors: &MapColors,
) {
    let Some(entry) = app.entries.get(idx) else { return };
    match app.thumb_tex.get(&entry.path) {
        Some(texture) => circular_image(
            painter,
            texture,
            center,
            radius,
            Stroke::new(1.0, Color32::from_white_alpha(150)),
        ),
        None => {
            painter.circle_filled(center, radius, colors.cluster_text.gamma_multiply(0.18));
            painter.circle_stroke(center, radius, Stroke::new(1.2, palette.accent));
        }
    }
    if app.entry_index() == Some(idx) {
        painter.circle_stroke(center, radius + 2.0, Stroke::new(1.8, palette.accent));
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
    let zoom = app.map.zoom;
    for cluster in clusters {
        let center = egui::pos2(rect.min.x + cluster.x, rect.min.y + cluster.y);
        let (shown, tile_r, positions) = cluster_layout(zoom, cluster.members.len());
        // 轻微背板阴影,让缩略图叠在一起时更清楚
        if shown > 1 {
            let bounds = cluster_bounds(zoom, cluster.members.len(), center);
            painter.rect_filled(bounds.expand(-2.0), 9.0, Color32::from_black_alpha(38));
        }
        for (i, (dx, dy)) in positions.iter().enumerate() {
            let Some(&idx) = cluster.members.get(i) else { continue };
            draw_photo_tile(
                app,
                painter,
                center + Vec2::new(*dx, *dy),
                tile_r,
                idx,
                palette,
                colors,
            );
        }
        let remaining = cluster.members.len().saturating_sub(shown);
        if remaining > 0 {
            let last = *positions.last().unwrap_or(&(0.0, 0.0));
            let badge = center + Vec2::new(last.0 + tile_r * 0.95, last.1 + tile_r * 0.95);
            painter.circle_filled(badge, 9.0, Color32::from_black_alpha(185));
            painter.circle_stroke(badge, 9.0, Stroke::new(1.0, Color32::from_white_alpha(130)));
            painter.text(
                badge,
                Align2::CENTER_CENTER,
                format!("+{remaining}"),
                FontId::proportional(9.5),
                Color32::WHITE,
            );
        }
        // 单张照片:没有卡片背景时给一个柔和底,避免贴图边缘发脏
        if cluster.members.len() == 1 {
            let c = center;
            painter.circle_filled(c, tile_r + 1.5, Color32::from_black_alpha(18));
        }
    }
}

/// 交互:拖动平移、滚轮缩放、点击缩略图/聚合、双击进浏览。
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
            if units.abs() > 0.5 {
                app.tile_failed.clear();
            }
        }
    }

    let now = ui.input(|i| i.time);
    for (ci, cluster) in clusters.iter().enumerate() {
        let center = egui::pos2(rect.min.x + cluster.x, rect.min.y + cluster.y);
        let hit_rect = cluster_bounds(app.map.zoom, cluster.members.len(), center);
        let hit = ui.interact(
            hit_rect,
            ui.scope_id().with(("map-cluster", ci)),
            Sense::click(),
        );
        if hit.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if hit.clicked() {
            if cluster.members.len() == 1 {
                let idx = cluster.members[0];
                app.select_with(idx, false, false);
                if hit.double_clicked() {
                    app.map_popup = None;
                    app.mode = Mode::Viewer;
                    app.load_current(ui.ctx());
                }
            } else if hit.double_clicked() {
                // 双击聚合:放大展开
                let world = app.map.screen_to_world(size, (cluster.x as f64, cluster.y as f64));
                app.map.center = geo::world_to_lonlat(world);
                app.map.zoom = (app.map.zoom + 1.5).min(app.max_map_zoom());
                app.tile_failed.clear();
                app.map_popup = None;
            } else {
                // 单击聚合:打开照片网格弹层
                app.map_popup = Some(MapPopup {
                    members: cluster.members.clone(),
                    anchor: center,
                    opened_at: now,
                });
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

/// 聚合点弹层:缩略图网格,单击选中、双击进入浏览。
fn draw_popup(app: &mut EasyShowApp, ui: &egui::Ui) {
    let Some(popup) = app.map_popup.clone() else { return };
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let palette = Palette::for_ui(ui);
    let colors = colors(ui.visuals().dark_mode);
    let text_color = ui.visuals().text_color();

    let members: Vec<usize> = popup.members.iter().take(POPUP_LIMIT).copied().collect();
    let tiles: Vec<(usize, PathBuf, Option<egui::TextureHandle>)> = members
        .iter()
        .filter_map(|&idx| {
            let entry = app.entries.get(idx)?;
            Some((idx, entry.path.clone(), app.thumb_tex.get(&entry.path).cloned()))
        })
        .collect();
    if tiles.is_empty() {
        app.map_popup = None;
        return;
    }

    let tile = 62.0;
    let gap = 8.0;
    let cols = ((tiles.len() as f32).sqrt().ceil() as usize).clamp(1, 4);
    let rows = (tiles.len() + cols - 1) / cols;
    let width = cols as f32 * (tile + gap) + gap;
    let height = rows as f32 * (tile + gap) + gap + 26.0;
    let screen = ctx.content_rect();
    let pos = egui::pos2(
        popup.anchor.x.clamp(screen.left() + 8.0, (screen.right() - width - 8.0).max(screen.left() + 8.0)),
        popup.anchor.y.clamp(screen.top() + 8.0, (screen.bottom() - height - 8.0).max(screen.top() + 8.0)),
    );

    let mut close = false;
    let mut select: Option<usize> = None;
    let mut open: Option<usize> = None;
    let area = egui::Area::new(ui.make_persistent_id("easyshow-map-popup"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(&ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(rect, 10.0, palette.card_bg);
            painter.rect_stroke(rect, 10.0, Stroke::new(1.0, colors.border), StrokeKind::Inside);
            painter.text(
                rect.left_top() + Vec2::new(12.0, 9.0),
                Align2::LEFT_TOP,
                format!("{} {}", tiles.len(), tr(app.lang, Key::Photos)),
                FontId::proportional(12.5),
                text_color,
            );
            let close_rect = Rect::from_min_size(
                egui::pos2(rect.right() - 26.0, rect.top() + 6.0),
                Vec2::splat(18.0),
            );
            painter.text(
                close_rect.center(),
                Align2::CENTER_CENTER,
                "×",
                FontId::proportional(15.0),
                palette.text_dim,
            );
            if ui
                .interact(close_rect, ui.scope_id().with("map-popup-close"), Sense::click())
                .clicked()
            {
                close = true;
            }
            for (i, (idx, _path, texture)) in tiles.iter().enumerate() {
                let row = i / cols;
                let col = i % cols;
                let center = egui::pos2(
                    rect.left() + gap + tile / 2.0 + col as f32 * (tile + gap),
                    rect.top() + 26.0 + gap / 2.0 + tile / 2.0 + row as f32 * (tile + gap),
                );
                let radius = tile / 2.0 - 2.0;
                match texture {
                    Some(texture) => circular_image(
                        painter,
                        texture,
                        center,
                        radius,
                        Stroke::new(1.0, colors.border),
                    ),
                    None => {
                        painter.circle_filled(center, radius, colors.cluster_text.gamma_multiply(0.15));
                        painter.circle_stroke(center, radius, Stroke::new(1.2, palette.accent));
                    }
                }
                if app.entry_index() == Some(*idx) {
                    painter.circle_stroke(center, radius + 2.0, Stroke::new(1.8, palette.accent));
                }
                let hit = ui.interact(
                    Rect::from_center_size(center, Vec2::splat(tile)),
                    ui.scope_id().with(("map-popup-item", i)),
                    Sense::click(),
                );
                if hit.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if hit.clicked() {
                    if hit.double_clicked() {
                        open = Some(*idx);
                    } else {
                        select = Some(*idx);
                    }
                }
                if let Some(entry) = app.entries.get(*idx) {
                    hit.on_hover_text(file_name(&entry.path));
                }
            }
        });

    if close {
        app.map_popup = None;
        return;
    }
    if let Some(idx) = select {
        app.select_with(idx, false, false);
    }
    if let Some(idx) = open {
        app.select_with(idx, false, false);
        app.map_popup = None;
        app.mode = Mode::Viewer;
        app.load_current(&ctx);
    }
    // 点击弹层之外 / Esc 关闭
    let clicked_outside = ctx.input(|i| i.pointer.any_click())
        && ctx
            .input(|i| i.pointer.interact_pos())
            .map(|p| !area.response.rect.contains(p))
            .unwrap_or(false);
    if clicked_outside && now > popup.opened_at + 0.05 {
        app.map_popup = None;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.map_popup = None;
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

    // 聚合缩略图:只为实际展示的成员请求/保留缩略图
    let mut needed: Vec<PathBuf> = Vec::new();
    for cluster in &clusters {
        let (shown, _, _) = cluster_layout(app.map.zoom, cluster.members.len());
        for &idx in cluster.members.iter().take(shown) {
            if let Some(entry) = app.entries.get(idx) {
                needed.push(entry.path.clone());
            }
        }
    }
    if let Some(popup) = &app.map_popup {
        for &idx in popup.members.iter().take(POPUP_LIMIT) {
            if let Some(entry) = app.entries.get(idx) {
                needed.push(entry.path.clone());
            }
        }
    }
    app.set_visible_thumbs(needed.clone());
    for path in &needed {
        app.request_thumb(path);
    }

    draw_clusters(app, &painter, rect, &clusters, palette, &map_colors);

    // 扫描进度 / 空态 / 离线提示 / 交互提示
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
    painter.text(
        egui::pos2(rect.left() + 12.0, rect.bottom() - 8.0),
        Align2::LEFT_BOTTOM,
        tr(lang, Key::MapHint),
        FontId::proportional(10.5),
        palette.text_dim,
    );

    interact(app, ui, &response, rect, &clusters);
    draw_popup(app, ui);
}
