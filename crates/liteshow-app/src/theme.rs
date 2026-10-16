//! 视觉主题:深/浅两套,默认跟随系统。
//!
//! egui 的两套 Visuals 各注册一次,由 ThemePreference 决定使用哪套;
//! 画布/网格等自绘部分用 [`Palette`] 取色(按当前 visuals.dark_mode 选择)。

use eframe::egui::{self, Color32};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

/// 自绘组件的取色板(不经过 egui widgets 的那部分)。
pub struct Palette {
    pub accent: Color32,
    pub canvas_bg: Color32,
    pub card_bg: Color32,
    pub thumb_bg: Color32,
    pub text_dim: Color32,
}

impl Palette {
    pub const DARK: Palette = Palette {
        accent: Color32::from_rgb(0x4d, 0xa3, 0xff),
        canvas_bg: Color32::from_rgb(0x0c, 0x0e, 0x12),
        card_bg: Color32::from_rgb(0x17, 0x1a, 0x20),
        thumb_bg: Color32::from_rgb(0x0e, 0x11, 0x16),
        text_dim: Color32::from_rgb(0x9a, 0xa4, 0xb5),
    };

    pub const LIGHT: Palette = Palette {
        accent: Color32::from_rgb(0x1d, 0x7b, 0xf0),
        canvas_bg: Color32::from_rgb(0xe9, 0xec, 0xf2),
        card_bg: Color32::from_rgb(0xff, 0xff, 0xff),
        thumb_bg: Color32::from_rgb(0xef, 0xf1, 0xf5),
        text_dim: Color32::from_rgb(0x6e, 0x77, 0x88),
    };

    pub fn for_dark(dark: bool) -> &'static Palette {
        if dark {
            &Self::DARK
        } else {
            &Self::LIGHT
        }
    }

    pub fn for_ui(ui: &egui::Ui) -> &'static Palette {
        Self::for_dark(ui.visuals().dark_mode)
    }
}

pub fn install(ctx: &egui::Context) {
    ctx.set_visuals_of(egui::Theme::Dark, dark_visuals());
    ctx.set_visuals_of(egui::Theme::Light, light_visuals());
    ctx.set_theme(egui::ThemePreference::System); // 缺省:跟随系统

    ctx.all_styles_mut(|style| {
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    });
}

/// 切换主题偏好(立即生效)。
pub fn apply_preference(ctx: &egui::Context, mode: ThemeMode) {
    ctx.set_theme(match mode {
        ThemeMode::System => egui::ThemePreference::System,
        ThemeMode::Light => egui::ThemePreference::Light,
        ThemeMode::Dark => egui::ThemePreference::Dark,
    });
}

fn rounded(visuals: &mut egui::Visuals) {
    // Apple 风格:按钮悬停/按下用圆角药丸
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(8);
    }
    // 目录树缩进不画竖线(资源管理器风格)
    visuals.indent_has_left_vline = false;
}

fn dark_visuals() -> egui::Visuals {
    let accent = Palette::DARK.accent;
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(0x14, 0x16, 0x1b);
    v.window_fill = Color32::from_rgb(0x19, 0x1c, 0x22);
    v.extreme_bg_color = Color32::from_rgb(0x0c, 0x0e, 0x12);
    v.faint_bg_color = Color32::from_rgb(0x1b, 0x1f, 0x26);
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0x4d, 0xa3, 0xff, 90);
    v.selection.stroke = egui::Stroke::new(1.0, accent);
    v.hyperlink_color = accent;
    v.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 18);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 18);
    v.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(0x4d, 0xa3, 0xff, 60);
    v.widgets.active.weak_bg_fill = Color32::from_rgba_unmultiplied(0x4d, 0xa3, 0xff, 60);
    rounded(&mut v);
    v
}

fn light_visuals() -> egui::Visuals {
    let accent = Palette::LIGHT.accent;
    let mut v = egui::Visuals::light();
    v.panel_fill = Color32::from_rgb(0xf2, 0xf3, 0xf6);
    v.window_fill = Color32::from_rgb(0xff, 0xff, 0xff);
    v.extreme_bg_color = Color32::from_rgb(0xe9, 0xec, 0xf2);
    v.faint_bg_color = Color32::from_rgb(0xee, 0xf0, 0xf4);
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0x1d, 0x7b, 0xf0, 55);
    v.selection.stroke = egui::Stroke::new(1.0, accent);
    v.hyperlink_color = accent;
    v.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 14);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 14);
    v.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(0x1d, 0x7b, 0xf0, 45);
    v.widgets.active.weak_bg_fill = Color32::from_rgba_unmultiplied(0x1d, 0x7b, 0xf0, 45);
    rounded(&mut v);
    v
}
