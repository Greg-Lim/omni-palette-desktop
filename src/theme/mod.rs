use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn egui_preference(self) -> egui::ThemePreference {
        match self {
            Self::System => egui::ThemePreference::System,
            Self::Light => egui::ThemePreference::Light,
            Self::Dark => egui::ThemePreference::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppTheme {
    pub palette: PaletteTheme,
    pub settings: SettingsTheme,
}

impl AppTheme {
    pub fn for_egui_theme(theme: egui::Theme) -> Self {
        match theme {
            egui::Theme::Dark => Self::warm_futurism_dark(),
            egui::Theme::Light => Self::warm_futurism_light(),
        }
    }

    fn warm_futurism_dark() -> Self {
        Self {
            palette: PaletteTheme {
                card_bg: egui::Color32::from_rgb(30, 24, 18),
                border: egui::Color32::from_rgb(89, 61, 36),
                search_bg: egui::Color32::from_rgb(38, 30, 22),
                search_border: egui::Color32::from_rgb(122, 82, 45),
                row_hover: egui::Color32::from_rgb(52, 39, 27),
                row_selected: egui::Color32::from_rgb(76, 45, 20),
                row_selected_border: egui::Color32::from_rgb(255, 149, 48),
                text_primary: egui::Color32::from_rgb(245, 232, 210),
                text_muted: egui::Color32::from_rgb(156, 126, 91),
                text_selected: egui::Color32::from_rgb(255, 246, 229),
                textbox_text: egui::Color32::from_rgb(250, 238, 218),
                textbox_hint: egui::Color32::from_rgb(154, 122, 85),
                textbox_cursor: egui::Color32::from_rgb(255, 149, 48),
                text_match: egui::Color32::from_rgb(255, 199, 94),
                text_match_selected: egui::Color32::from_rgb(255, 231, 162),
                shortcut_selected: egui::Color32::from_rgb(255, 208, 154),
                favorite: egui::Color32::from_rgb(255, 116, 107),
                favorite_selected: egui::Color32::from_rgb(255, 190, 181),
            },
            settings: SettingsTheme {
                bg: egui::Color32::from_rgb(18, 14, 10),
                sidebar_bg: egui::Color32::from_rgb(24, 18, 13),
                surface: egui::Color32::from_rgb(30, 24, 18),
                surface_alt: egui::Color32::from_rgb(38, 30, 22),
                input_bg: egui::Color32::from_rgb(22, 17, 12),
                input_hover: egui::Color32::from_rgb(52, 39, 27),
                border: egui::Color32::from_rgb(89, 61, 36),
                border_soft: egui::Color32::from_rgb(65, 47, 32),
                accent: egui::Color32::from_rgb(255, 149, 48),
                accent_soft: egui::Color32::from_rgb(122, 64, 24),
                text_primary: egui::Color32::from_rgb(245, 232, 210),
                text_secondary: egui::Color32::from_rgb(199, 174, 137),
                text_muted: egui::Color32::from_rgb(130, 105, 77),
                text_on_accent: egui::Color32::from_rgb(255, 246, 229),
                warning: egui::Color32::from_rgb(255, 194, 105),
                error: egui::Color32::from_rgb(239, 84, 67),
                success: egui::Color32::from_rgb(94, 211, 149),
                nav_selected: egui::Color32::from_rgb(76, 45, 20),
                info_bg: egui::Color32::from_rgb(52, 39, 27),
                warning_bg: egui::Color32::from_rgb(61, 43, 20),
                error_bg: egui::Color32::from_rgb(58, 29, 25),
                primary_button_text: egui::Color32::from_rgb(255, 238, 218),
                primary_button_bg: egui::Color32::from_rgb(92, 51, 20),
                primary_button_border: egui::Color32::from_rgb(210, 119, 40),
                danger_button_text: egui::Color32::from_rgb(255, 190, 181),
                danger_button_bg: egui::Color32::from_rgb(58, 29, 25),
                danger_button_border: egui::Color32::from_rgb(185, 74, 64),
                shadow: egui::Color32::from_black_alpha(100),
            },
        }
    }

    fn warm_futurism_light() -> Self {
        Self {
            palette: PaletteTheme {
                card_bg: egui::Color32::from_rgb(255, 248, 239),
                border: egui::Color32::from_rgb(217, 185, 142),
                search_bg: egui::Color32::from_rgb(255, 253, 249),
                search_border: egui::Color32::from_rgb(205, 164, 111),
                row_hover: egui::Color32::from_rgb(255, 235, 207),
                row_selected: egui::Color32::from_rgb(255, 217, 174),
                row_selected_border: egui::Color32::from_rgb(217, 109, 28),
                text_primary: egui::Color32::from_rgb(43, 30, 21),
                text_muted: egui::Color32::from_rgb(141, 111, 83),
                text_selected: egui::Color32::from_rgb(43, 30, 21),
                textbox_text: egui::Color32::from_rgb(43, 30, 21),
                textbox_hint: egui::Color32::from_rgb(141, 111, 83),
                textbox_cursor: egui::Color32::from_rgb(217, 109, 28),
                text_match: egui::Color32::from_rgb(174, 82, 12),
                text_match_selected: egui::Color32::from_rgb(115, 55, 12),
                shortcut_selected: egui::Color32::from_rgb(104, 76, 50),
                favorite: egui::Color32::from_rgb(201, 66, 68),
                favorite_selected: egui::Color32::from_rgb(145, 43, 45),
            },
            settings: SettingsTheme {
                bg: egui::Color32::from_rgb(246, 239, 230),
                sidebar_bg: egui::Color32::from_rgb(239, 226, 208),
                surface: egui::Color32::from_rgb(255, 248, 239),
                surface_alt: egui::Color32::from_rgb(242, 226, 205),
                input_bg: egui::Color32::from_rgb(255, 253, 249),
                input_hover: egui::Color32::from_rgb(255, 235, 207),
                border: egui::Color32::from_rgb(217, 185, 142),
                border_soft: egui::Color32::from_rgb(234, 215, 191),
                accent: egui::Color32::from_rgb(217, 109, 28),
                accent_soft: egui::Color32::from_rgb(242, 196, 145),
                text_primary: egui::Color32::from_rgb(43, 30, 21),
                text_secondary: egui::Color32::from_rgb(104, 76, 50),
                text_muted: egui::Color32::from_rgb(141, 111, 83),
                text_on_accent: egui::Color32::from_rgb(43, 30, 21),
                warning: egui::Color32::from_rgb(163, 93, 0),
                error: egui::Color32::from_rgb(191, 62, 55),
                success: egui::Color32::from_rgb(34, 122, 82),
                nav_selected: egui::Color32::from_rgb(255, 217, 174),
                info_bg: egui::Color32::from_rgb(255, 235, 207),
                warning_bg: egui::Color32::from_rgb(255, 230, 186),
                error_bg: egui::Color32::from_rgb(255, 224, 219),
                primary_button_text: egui::Color32::from_rgb(255, 248, 239),
                primary_button_bg: egui::Color32::from_rgb(174, 82, 12),
                primary_button_border: egui::Color32::from_rgb(217, 109, 28),
                danger_button_text: egui::Color32::from_rgb(255, 239, 236),
                danger_button_bg: egui::Color32::from_rgb(168, 50, 45),
                danger_button_border: egui::Color32::from_rgb(191, 62, 55),
                shadow: egui::Color32::from_black_alpha(55),
            },
        }
    }

    fn to_style(self, theme: egui::Theme) -> egui::Style {
        let mut style = theme.default_style();
        let settings = self.settings;
        let palette = self.palette;
        let visuals = &mut style.visuals;

        visuals.window_fill = settings.bg;
        visuals.panel_fill = settings.bg;
        visuals.extreme_bg_color = settings.input_bg;
        visuals.faint_bg_color = settings.surface;
        visuals.override_text_color = Some(settings.text_primary);
        visuals.hyperlink_color = settings.accent;
        visuals.selection.bg_fill = settings.accent_soft;
        visuals.selection.stroke = egui::Stroke::new(1.0, settings.accent);
        visuals.text_cursor.stroke = egui::Stroke::new(2.0, settings.accent);

        visuals.widgets.noninteractive.bg_fill = settings.surface;
        visuals.widgets.noninteractive.fg_stroke.color = settings.text_secondary;
        visuals.widgets.inactive.bg_fill = palette.search_bg;
        visuals.widgets.inactive.weak_bg_fill = settings.surface_alt;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, settings.border);
        visuals.widgets.inactive.fg_stroke.color = settings.text_primary;
        visuals.widgets.hovered.bg_fill = settings.input_hover;
        visuals.widgets.hovered.weak_bg_fill = settings.input_hover;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, settings.accent);
        visuals.widgets.hovered.fg_stroke.color = settings.text_primary;
        visuals.widgets.active.bg_fill = settings.accent_soft;
        visuals.widgets.active.weak_bg_fill = settings.accent_soft;
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, settings.accent);
        visuals.widgets.active.fg_stroke.color = settings.text_on_accent;
        visuals.widgets.open.bg_fill = settings.accent_soft;
        visuals.widgets.open.weak_bg_fill = settings.accent_soft;
        visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, settings.accent);
        visuals.widgets.open.fg_stroke.color = settings.text_on_accent;

        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(0);
        style
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteTheme {
    pub card_bg: egui::Color32,
    pub border: egui::Color32,
    pub search_bg: egui::Color32,
    pub search_border: egui::Color32,
    pub row_hover: egui::Color32,
    pub row_selected: egui::Color32,
    pub row_selected_border: egui::Color32,
    pub text_primary: egui::Color32,
    pub text_muted: egui::Color32,
    pub text_selected: egui::Color32,
    pub textbox_text: egui::Color32,
    pub textbox_hint: egui::Color32,
    pub textbox_cursor: egui::Color32,
    pub text_match: egui::Color32,
    pub text_match_selected: egui::Color32,
    pub shortcut_selected: egui::Color32,
    pub favorite: egui::Color32,
    pub favorite_selected: egui::Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsTheme {
    pub bg: egui::Color32,
    pub sidebar_bg: egui::Color32,
    pub surface: egui::Color32,
    pub surface_alt: egui::Color32,
    pub input_bg: egui::Color32,
    pub input_hover: egui::Color32,
    pub border: egui::Color32,
    pub border_soft: egui::Color32,
    pub accent: egui::Color32,
    pub accent_soft: egui::Color32,
    pub text_primary: egui::Color32,
    pub text_secondary: egui::Color32,
    pub text_muted: egui::Color32,
    pub text_on_accent: egui::Color32,
    pub warning: egui::Color32,
    pub error: egui::Color32,
    pub success: egui::Color32,
    pub nav_selected: egui::Color32,
    pub info_bg: egui::Color32,
    pub warning_bg: egui::Color32,
    pub error_bg: egui::Color32,
    pub primary_button_text: egui::Color32,
    pub primary_button_bg: egui::Color32,
    pub primary_button_border: egui::Color32,
    pub danger_button_text: egui::Color32,
    pub danger_button_bg: egui::Color32,
    pub danger_button_border: egui::Color32,
    pub shadow: egui::Color32,
}

pub fn apply_app_theme(ctx: &egui::Context, mode: ThemeMode) {
    ctx.set_style_of(
        egui::Theme::Dark,
        AppTheme::for_egui_theme(egui::Theme::Dark).to_style(egui::Theme::Dark),
    );
    ctx.set_style_of(
        egui::Theme::Light,
        AppTheme::for_egui_theme(egui::Theme::Light).to_style(egui::Theme::Light),
    );
    ctx.options_mut(|options| options.fallback_theme = egui::Theme::Dark);
    ctx.set_theme(mode.egui_preference());
}

pub fn current_app_theme(ctx: &egui::Context) -> AppTheme {
    AppTheme::for_egui_theme(ctx.theme())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_maps_to_egui_preference() {
        assert_eq!(
            ThemeMode::System.egui_preference(),
            egui::ThemePreference::System
        );
        assert_eq!(
            ThemeMode::Light.egui_preference(),
            egui::ThemePreference::Light
        );
        assert_eq!(
            ThemeMode::Dark.egui_preference(),
            egui::ThemePreference::Dark
        );
    }

    #[test]
    fn app_theme_uses_distinct_light_and_dark_tokens() {
        let light = AppTheme::for_egui_theme(egui::Theme::Light);
        let dark = AppTheme::for_egui_theme(egui::Theme::Dark);

        assert_ne!(light.palette.card_bg, dark.palette.card_bg);
        assert_ne!(light.settings.bg, dark.settings.bg);
        assert_eq!(dark.palette.row_selected_border, dark.settings.accent);
        assert_eq!(light.palette.row_selected_border, light.settings.accent);
    }

    #[test]
    fn app_style_keeps_horizontal_widget_spacing() {
        let style = AppTheme::for_egui_theme(egui::Theme::Light).to_style(egui::Theme::Light);

        assert_eq!(style.spacing.item_spacing, egui::vec2(8.0, 6.0));
    }
}
