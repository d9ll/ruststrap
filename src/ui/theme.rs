use eframe::egui::{self, Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Visuals};

pub const BG: Color32 = Color32::from_rgb(15, 17, 23);
pub const PANEL: Color32 = Color32::from_rgb(21, 24, 32);
pub const CARD: Color32 = Color32::from_rgb(29, 33, 43);
pub const CARD_HOVER: Color32 = Color32::from_rgb(36, 41, 54);
pub const ACCENT: Color32 = Color32::from_rgb(92, 137, 255);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(116, 156, 255);
pub const TEXT: Color32 = Color32::from_rgb(232, 235, 243);
pub const MUTED: Color32 = Color32::from_rgb(145, 154, 173);
pub const DANGER: Color32 = Color32::from_rgb(238, 99, 104);
pub const SUCCESS: Color32 = Color32::from_rgb(88, 204, 141);
pub const BORDER: Color32 = Color32::from_rgb(45, 51, 65);

pub fn apply(ctx: &egui::Context, theme: &str) {
    let light = theme.eq_ignore_ascii_case("light");
    let mut style = (*ctx.style()).clone();

    let mut visuals = if light {
        Visuals::light()
    } else {
        Visuals::dark()
    };

    visuals.override_text_color = Some(if light {
        Color32::from_rgb(28, 32, 40)
    } else {
        TEXT
    });
    visuals.panel_fill = if light {
        Color32::from_rgb(244, 246, 250)
    } else {
        BG
    };
    visuals.window_fill = visuals.panel_fill;
    visuals.extreme_bg_color = if light {
        Color32::from_rgb(232, 235, 241)
    } else {
        Color32::from_rgb(11, 13, 18)
    };
    visuals.faint_bg_color = if light {
        Color32::from_rgb(236, 239, 245)
    } else {
        Color32::from_rgb(25, 28, 37)
    };

    visuals.widgets.noninteractive.bg_fill = if light {
        Color32::from_rgb(236, 239, 245)
    } else {
        PANEL
    };
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0f32, visuals.override_text_color.unwrap());
    visuals.widgets.inactive.bg_fill = if light {
        Color32::from_rgb(230, 234, 241)
    } else {
        CARD
    };
    visuals.widgets.inactive.weak_bg_fill = visuals.widgets.inactive.bg_fill;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0f32,
        if light {
            Color32::from_rgb(52, 58, 70)
        } else {
            TEXT
        },
    );
    visuals.widgets.hovered.bg_fill = if light {
        Color32::from_rgb(219, 225, 235)
    } else {
        CARD_HOVER
    };
    visuals.widgets.hovered.weak_bg_fill = visuals.widgets.hovered.bg_fill;
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0f32, Color32::WHITE);
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.45);
    visuals.selection.stroke = Stroke::new(1.0f32, Color32::WHITE);
    visuals.window_stroke = Stroke::new(1.0f32, BORDER);

    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.rounding = Rounding::same(8.0);
    }

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.interact_size.y = 30.0;

    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(26.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(14.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );

    ctx.set_style(style);
}
