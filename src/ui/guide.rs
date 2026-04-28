use crate::domain::hotkey::KeyboardShortcut;
use crate::platform::ui_support::PlatformWindowToken;
use crate::ui::app::PaletteWorkArea;
use eframe::egui;
use std::time::{Duration, Instant};

const GUIDE_VIEWPORT_ID: &str = "omni_palette_guide";
const GUIDE_WIDTH: f32 = 560.0;
const GUIDE_HEIGHT: f32 = 218.0;
const GUIDE_PANEL_CORNER_RADIUS: u8 = 10;
const GUIDE_PANEL_STROKE_WIDTH: f32 = 1.0;
const GUIDE_PANEL_INSET: f32 = 1.0;
const GUIDE_PANEL_MARGIN_X: f32 = 56.0;
const GUIDE_PANEL_MARGIN_Y: f32 = 30.0;
const GUIDE_LABEL_FONT_SIZE: f32 = 25.0;
const GUIDE_KEYCAP_TOP_SPACE: f32 = 10.0;
const GUIDE_KEYCAP_HEIGHT: f32 = 72.0;
const GUIDE_KEYCAP_CORNER_RADIUS: u8 = 6;
const GUIDE_KEYCAP_STROKE_WIDTH: f32 = 2.0;
const GUIDE_KEYCAP_FONT_SIZE: f32 = 40.0;
const GUIDE_KEYCAP_SINGLE_WIDTH: f32 = 96.0;
const GUIDE_KEYCAP_MIN_WIDTH: f32 = 124.0;
const GUIDE_KEYCAP_MAX_WIDTH: f32 = 228.0;
const GUIDE_KEYCAP_CHAR_WIDTH: f32 = 34.0;
const GUIDE_KEYCAP_HORIZONTAL_PADDING: f32 = 56.0;
const GUIDE_KEYCAP_SPACING_X: f32 = 14.0;
const GUIDE_KEYCAP_SPACING_Y: f32 = 10.0;
const GUIDE_SEQUENCE_SEPARATOR_FONT_SIZE: f32 = 30.0;
const GUIDE_FALLBACK_TOP_SPACE: f32 = 14.0;
const GUIDE_FALLBACK_FONT_SIZE: f32 = 24.0;
const GUIDE_FALLBACK_SUFFIX: &str = "to run for me";
const GUIDE_DEFAULT_MONITOR_WIDTH: f32 = 1920.0;
const GUIDE_DEFAULT_MONITOR_HEIGHT: f32 = 1080.0;
const GUIDE_VERTICAL_POSITION_FACTOR: f32 = 0.45;

const GUIDE_TEXT_OVERRIDE: egui::Color32 = egui::Color32::from_rgb(17, 17, 17);
const GUIDE_PANEL_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(196, 196, 196, 224);
const GUIDE_PANEL_BORDER: egui::Color32 =
    egui::Color32::from_rgba_premultiplied(235, 235, 235, 150);
const GUIDE_LABEL_TEXT: egui::Color32 = egui::Color32::from_rgba_premultiplied(18, 18, 18, 245);
const GUIDE_SEQUENCE_SEPARATOR_TEXT: egui::Color32 =
    egui::Color32::from_rgba_premultiplied(24, 24, 24, 235);
const GUIDE_FALLBACK_TEXT: egui::Color32 = egui::Color32::from_rgba_premultiplied(20, 20, 20, 240);
const GUIDE_KEYCAP_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(94, 94, 94, 238);
const GUIDE_KEYCAP_BORDER: egui::Color32 = egui::Color32::from_rgba_premultiplied(48, 48, 48, 245);
const GUIDE_KEYCAP_TEXT: egui::Color32 = egui::Color32::from_rgba_premultiplied(250, 250, 250, 250);

pub(crate) const GUIDE_DURATION: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuideHint {
    pub target_window: Option<PlatformWindowToken>,
    pub shortcut: Option<KeyboardShortcut>,
}

#[derive(Debug, Clone)]
pub(crate) struct ActiveGuide {
    pub(crate) command_index: usize,
    pub(crate) label: String,
    pub(crate) shortcut_text: String,
    pub(crate) activation_hint: String,
    pub(crate) work_area: Option<PaletteWorkArea>,
    pub(crate) expires_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GuideShortcutPart {
    Key(String),
    SequenceSeparator,
}

pub(crate) fn close_guide_viewport(ctx: &egui::Context) {
    ctx.send_viewport_cmd_to(
        egui::ViewportId::from_hash_of(GUIDE_VIEWPORT_ID),
        egui::ViewportCommand::Close,
    );
}

pub(crate) fn show_guide_viewport(ctx: &egui::Context, guide: &ActiveGuide) {
    let position = guide_viewport_position(ctx, guide.work_area);
    let label = guide.label.clone();
    let shortcut_parts = parse_guide_shortcut(&guide.shortcut_text);
    let fallback_text = guide_fallback_text(&guide.activation_hint);

    ctx.show_viewport_deferred(
        egui::ViewportId::from_hash_of(GUIDE_VIEWPORT_ID),
        egui::ViewportBuilder::default()
            .with_title("Omni Palette Guide")
            .with_inner_size([GUIDE_WIDTH, GUIDE_HEIGHT])
            .with_position(position)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_mouse_passthrough(true)
            .with_active(false),
        move |ui, _class| {
            draw_guide_contents(ui, &label, &shortcut_parts, &fallback_text);
        },
    );
}

fn guide_viewport_position(ctx: &egui::Context, work_area: Option<PaletteWorkArea>) -> egui::Pos2 {
    let native_pixels_per_point = ctx
        .input(|i| i.viewport().native_pixels_per_point)
        .unwrap_or(1.0)
        .max(1.0);
    if let Some(work_area) = work_area {
        let work_area = work_area.to_points(native_pixels_per_point);
        return egui::pos2(
            work_area.left + ((work_area.width() - GUIDE_WIDTH) / 2.0).max(0.0),
            work_area.top
                + ((work_area.height() - GUIDE_HEIGHT) * GUIDE_VERTICAL_POSITION_FACTOR).max(0.0),
        );
    }

    let monitor_size = ctx
        .input(|i| i.viewport().monitor_size)
        .unwrap_or(egui::vec2(
            GUIDE_DEFAULT_MONITOR_WIDTH,
            GUIDE_DEFAULT_MONITOR_HEIGHT,
        ));
    egui::pos2(
        ((monitor_size.x - GUIDE_WIDTH) / 2.0).max(0.0),
        ((monitor_size.y - GUIDE_HEIGHT) * GUIDE_VERTICAL_POSITION_FACTOR).max(0.0),
    )
}

fn draw_guide_contents(
    ui: &mut egui::Ui,
    label: &str,
    parts: &[GuideShortcutPart],
    fallback: &str,
) {
    ui.visuals_mut().override_text_color = Some(GUIDE_TEXT_OVERRIDE);
    let rect = ui.max_rect();
    ui.painter().rect(
        rect.shrink(GUIDE_PANEL_INSET),
        egui::CornerRadius::same(GUIDE_PANEL_CORNER_RADIUS),
        GUIDE_PANEL_BG,
        egui::Stroke::new(GUIDE_PANEL_STROKE_WIDTH, GUIDE_PANEL_BORDER),
        egui::StrokeKind::Outside,
    );

    let inner = rect.shrink2(egui::vec2(GUIDE_PANEL_MARGIN_X, GUIDE_PANEL_MARGIN_Y));
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(GUIDE_LABEL_FONT_SIZE)
                    .color(GUIDE_LABEL_TEXT),
            );
            ui.add_space(GUIDE_KEYCAP_TOP_SPACE);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing =
                    egui::vec2(GUIDE_KEYCAP_SPACING_X, GUIDE_KEYCAP_SPACING_Y);
                for part in parts {
                    match part {
                        GuideShortcutPart::Key(key) => draw_keycap(ui, key),
                        GuideShortcutPart::SequenceSeparator => {
                            ui.label(
                                egui::RichText::new(",")
                                    .size(GUIDE_SEQUENCE_SEPARATOR_FONT_SIZE)
                                    .color(GUIDE_SEQUENCE_SEPARATOR_TEXT),
                            );
                        }
                    }
                }
            });
            ui.add_space(GUIDE_FALLBACK_TOP_SPACE);
            ui.label(
                egui::RichText::new(fallback)
                    .size(GUIDE_FALLBACK_FONT_SIZE)
                    .color(GUIDE_FALLBACK_TEXT),
            );
        });
    });
}

fn draw_keycap(ui: &mut egui::Ui, key: &str) {
    let desired_size = egui::vec2(keycap_width(key), GUIDE_KEYCAP_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(GUIDE_KEYCAP_CORNER_RADIUS),
        GUIDE_KEYCAP_BG,
        egui::Stroke::new(GUIDE_KEYCAP_STROKE_WIDTH, GUIDE_KEYCAP_BORDER),
        egui::StrokeKind::Outside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        key,
        egui::FontId::proportional(GUIDE_KEYCAP_FONT_SIZE),
        GUIDE_KEYCAP_TEXT,
    );
}

fn keycap_width(key: &str) -> f32 {
    if key.chars().count() <= 1 {
        GUIDE_KEYCAP_SINGLE_WIDTH
    } else {
        (key.chars().count() as f32 * GUIDE_KEYCAP_CHAR_WIDTH + GUIDE_KEYCAP_HORIZONTAL_PADDING)
            .clamp(GUIDE_KEYCAP_MIN_WIDTH, GUIDE_KEYCAP_MAX_WIDTH)
    }
}

fn parse_guide_shortcut(shortcut_text: &str) -> Vec<GuideShortcutPart> {
    let mut parts = Vec::new();
    for (sequence_index, chord) in shortcut_text.split(',').enumerate() {
        if sequence_index > 0 {
            parts.push(GuideShortcutPart::SequenceSeparator);
        }
        for key in chord
            .split('+')
            .map(format_guide_key)
            .filter(|key| !key.is_empty())
        {
            parts.push(GuideShortcutPart::Key(key));
        }
    }
    parts
}

fn format_guide_key(key: &str) -> String {
    let key = key.trim();
    if key.eq_ignore_ascii_case("ctrl") || key.eq_ignore_ascii_case("control") {
        "ctrl".to_string()
    } else if key.eq_ignore_ascii_case("shift") {
        "shift".to_string()
    } else if key.eq_ignore_ascii_case("alt") {
        "alt".to_string()
    } else if key.eq_ignore_ascii_case("win") || key.eq_ignore_ascii_case("windows") {
        "win".to_string()
    } else {
        key.to_string()
    }
}

fn guide_fallback_text(activation_hint: &str) -> String {
    let keys = activation_hint
        .split('+')
        .map(|key| format_guide_key(key).to_ascii_lowercase())
        .filter(|key| !key.is_empty())
        .collect::<Vec<_>>();
    format!("{} {}", keys.join(" + "), GUIDE_FALLBACK_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::{guide_fallback_text, parse_guide_shortcut, GuideShortcutPart, GUIDE_DURATION};

    #[test]
    fn guide_duration_is_eight_seconds() {
        assert_eq!(GUIDE_DURATION, std::time::Duration::from_secs(8));
    }

    #[test]
    fn guide_shortcut_parses_single_chord_into_keycaps() {
        assert_eq!(
            parse_guide_shortcut("Ctrl+T"),
            vec![
                GuideShortcutPart::Key("ctrl".to_string()),
                GuideShortcutPart::Key("T".to_string()),
            ]
        );
    }

    #[test]
    fn guide_shortcut_parses_sequences_with_separator() {
        assert_eq!(
            parse_guide_shortcut("Alt+J, I"),
            vec![
                GuideShortcutPart::Key("alt".to_string()),
                GuideShortcutPart::Key("J".to_string()),
                GuideShortcutPart::SequenceSeparator,
                GuideShortcutPart::Key("I".to_string()),
            ]
        );
    }

    #[test]
    fn guide_fallback_text_formats_activation_shortcut() {
        assert_eq!(
            guide_fallback_text("Ctrl+Shift+P"),
            "ctrl + shift + p to run for me"
        );
    }
}
