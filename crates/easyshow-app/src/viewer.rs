//! View transforms: pan/zoom state plus the pure math, unit-tested.
//!
//! Rotation and flip are applied to the pixel buffer before upload, so the
//! transform math here stays axis-aligned and cursor-anchored zoom is simple.

use eframe::egui::{Pos2, Rect, Vec2};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoomMode {
    /// Scale to fit the window (recomputed on resize).
    Fit,
    /// Explicit scale factor, 1.0 = 100 %.
    Fixed(f32),
}

#[derive(Debug, Clone)]
pub struct Viewer {
    pub mode: ZoomMode,
    /// Current scale: image pixels -> screen points.
    pub zoom: f32,
    /// Image center offset from viewport center, in screen points.
    pub offset: Vec2,
    /// User rotation in clockwise quarter turns (0..=3).
    pub rotation: u8,
    pub flip_h: bool,
}

impl Default for Viewer {
    fn default() -> Self {
        Self { mode: ZoomMode::Fit, zoom: 1.0, offset: Vec2::ZERO, rotation: 0, flip_h: false }
    }
}

impl Viewer {
    /// Zoom so that `image` fits inside `viewport` with a small margin.
    pub fn fit_zoom(image: Vec2, viewport: Vec2) -> f32 {
        if image.x <= 0.0 || image.y <= 0.0 || viewport.x <= 0.0 || viewport.y <= 0.0 {
            return 1.0;
        }
        ((viewport.x / image.x).min(viewport.y / image.y) * 0.96).max(0.01)
    }

    pub fn recompute_fit(&mut self, image: Vec2, viewport: Vec2) {
        self.zoom = Self::fit_zoom(image, viewport);
        self.offset = Vec2::ZERO;
    }

    /// Change zoom while keeping the image point currently under `cursor`
    /// fixed on screen. `viewport_center` and `cursor` share a coordinate
    /// space (any space works, since only differences are used).
    pub fn zoom_at(&mut self, new_zoom: f32, cursor: Pos2, viewport_center: Pos2) {
        let old_zoom = self.zoom.max(1e-6);
        let image_center = viewport_center + self.offset;
        let anchor = cursor - image_center; // vector image-center -> cursor
        let new_center = cursor - anchor * (new_zoom / old_zoom);
        self.zoom = new_zoom;
        self.offset = new_center - viewport_center;
    }

    /// Screen-space rectangle of the image, centered on `viewport_center`
    /// plus the current pan offset.
    pub fn image_rect(&self, image: Vec2, viewport_center: Pos2) -> Rect {
        Rect::from_center_size(viewport_center + self.offset, image * self.zoom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_zoom_uses_the_tighter_axis_with_margin() {
        let zoom = Viewer::fit_zoom(Vec2::new(1000.0, 500.0), Vec2::new(800.0, 600.0));
        assert!((zoom - 0.8 * 0.96).abs() < 1e-6);
    }

    #[test]
    fn zoom_at_keeps_the_cursor_anchor() {
        let mut v = Viewer { mode: ZoomMode::Fixed(1.0), zoom: 1.0, offset: Vec2::new(10.0, -5.0), ..Default::default() };
        let viewport_center = Pos2::new(400.0, 300.0);
        let cursor = Pos2::new(460.0, 340.0);

        // Image-space coordinate of the point under the cursor, before zoom.
        let before = (cursor - (viewport_center + v.offset)) / v.zoom;

        v.zoom_at(2.5, cursor, viewport_center);

        // The same image-space point must still be under the cursor.
        let after = (cursor - (viewport_center + v.offset)) / v.zoom;
        assert!((before - after).length() < 1e-4, "before={before:?} after={after:?}");
    }

    #[test]
    fn zoom_at_center_keeps_offset_direction() {
        let mut v = Viewer { mode: ZoomMode::Fixed(1.0), zoom: 2.0, offset: Vec2::ZERO, ..Default::default() };
        let center = Pos2::new(100.0, 100.0);
        v.zoom_at(1.0, center, center);
        assert_eq!(v.offset, Vec2::ZERO);
        assert_eq!(v.zoom, 1.0);
    }

    #[test]
    fn image_rect_tracks_offset() {
        let v = Viewer { mode: ZoomMode::Fixed(1.0), zoom: 0.5, offset: Vec2::new(20.0, 10.0), ..Default::default() };
        let center = Pos2::new(100.0, 100.0);
        let rect = v.image_rect(Vec2::new(400.0, 200.0), center);
        assert_eq!(rect.center(), Pos2::new(120.0, 110.0));
        assert_eq!(rect.width(), 200.0);
        assert_eq!(rect.height(), 100.0);
    }
}
