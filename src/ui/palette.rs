use crate::core::command_filter::{
    filter_commands, initial_filtered_commands, FilterableCommand, FilteredCommand,
};
use crate::core::search::MatchRange;
use crate::domain::action::{CommandPriority, FocusState};
use crate::ui::app::Command;
use eframe::egui;
use eframe::egui::text::LayoutJob;

pub(crate) const PALETTE_WIDTH: f32 = 780.0;
pub(crate) const MAX_FILTERED_COMMANDS: usize = 18;
pub(crate) const MAX_VISIBLE_COMMAND_ROWS: usize = 10;
pub(crate) const ROW_HEIGHT: f32 = 38.0;
pub(crate) const SETTINGS_DIVIDER_HEIGHT: f32 = 13.0;
pub(crate) const FIXED_ACTION_ROW_HEIGHT: f32 = 30.0;
pub(crate) const ESTIMATED_VISIBLE_ROW_HEIGHT: f32 = 31.0;
pub(crate) const PALETTE_FRAME_RADIUS: u8 = 8;
pub(crate) const PALETTE_FRAME_MARGIN: i8 = 10;
pub(crate) const PALETTE_SEARCH_RADIUS: u8 = 6;
pub(crate) const PALETTE_SEARCH_MARGIN_X: i8 = 12;
pub(crate) const PALETTE_SEARCH_MARGIN_Y: i8 = 14;
pub(crate) const PALETTE_SEARCH_PROMPT_SIZE: f32 = 18.0;
pub(crate) const PALETTE_SEARCH_PROMPT_LEFT_SPACE: f32 = 2.0;
pub(crate) const PALETTE_SEARCH_PROMPT_RIGHT_SPACE: f32 = 6.0;
pub(crate) const PALETTE_SEARCH_HEIGHT: f32 = 28.0;
pub(crate) const PALETTE_RESULTS_TOP_SPACE: f32 = 8.0;
pub(crate) const PALETTE_EMPTY_ROW_MARGIN: i8 = 12;
pub(crate) const PALETTE_EMPTY_ROW_RADIUS: u8 = 6;
pub(crate) const PALETTE_EMPTY_TEXT_SIZE: f32 = 14.5;
const PALETTE_ROW_RADIUS: u8 = 6;
const PALETTE_ROW_MARGIN_X: f32 = 12.0;
const PALETTE_ROW_MARGIN_Y: f32 = 8.0;
const PALETTE_FIXED_ROW_MARGIN_Y: f32 = 4.0;
const PALETTE_ROW_LABEL_SIZE: f32 = 15.5;
const PALETTE_SHORTCUT_SIZE: f32 = 12.5;
const PALETTE_FAVORITE_SPACE: f32 = 8.0;
const PALETTE_DIVIDER_MARGIN_X: f32 = 8.0;
pub(crate) const PALETTE_BORDER_WIDTH: f32 = 1.0;
pub(crate) const PALETTE_CURSOR_WIDTH: f32 = 2.0;

pub(crate) const PALETTE_WINDOW_BG: egui::Color32 = egui::Color32::from_rgb(30, 30, 30);
pub(crate) const PALETTE_CARD_BG: egui::Color32 = egui::Color32::from_rgb(37, 37, 38);
pub(crate) const PALETTE_BORDER: egui::Color32 = egui::Color32::from_rgb(69, 69, 69);
pub(crate) const PALETTE_SEARCH_BG: egui::Color32 = egui::Color32::from_rgb(30, 30, 30);
pub(crate) const PALETTE_SEARCH_BORDER: egui::Color32 = egui::Color32::from_rgb(82, 82, 82);
pub(crate) const PALETTE_ROW_HOVER: egui::Color32 = egui::Color32::from_rgb(43, 43, 43);
pub(crate) const PALETTE_ROW_SELECTED: egui::Color32 = egui::Color32::from_rgb(9, 71, 113);
pub(crate) const PALETTE_ROW_SELECTED_BORDER: egui::Color32 = egui::Color32::from_rgb(55, 148, 255);
pub(crate) const PALETTE_TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(204, 204, 204);
pub(crate) const PALETTE_TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(150, 150, 150);
pub(crate) const PALETTE_TEXT_SELECTED: egui::Color32 = egui::Color32::WHITE;
pub(crate) const PALETTE_TEXTBOX_TEXT: egui::Color32 = egui::Color32::from_rgb(230, 230, 230);
pub(crate) const PALETTE_TEXTBOX_HINT: egui::Color32 = egui::Color32::from_rgb(120, 120, 120);
pub(crate) const PALETTE_TEXTBOX_CURSOR: egui::Color32 = egui::Color32::from_rgb(0, 122, 204);
const PALETTE_TEXT_MATCH: egui::Color32 = egui::Color32::from_rgb(255, 213, 122);
const PALETTE_TEXT_MATCH_SELECTED: egui::Color32 = egui::Color32::from_rgb(255, 238, 180);
const PALETTE_SHORTCUT_SELECTED: egui::Color32 = egui::Color32::from_rgb(180, 200, 230);
const PALETTE_FAVORITE_ICON: &str = "\u{2665}";
const PALETTE_FAVORITE_SIZE: f32 = 12.0;
const PALETTE_FAVORITE_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 106, 148);
const PALETTE_FAVORITE_SELECTED: egui::Color32 = egui::Color32::from_rgb(255, 190, 214);

pub(crate) const FIXED_PALETTE_ACTIONS: [FixedPaletteAction; 2] = [
    FixedPaletteAction::RefreshExtensions,
    FixedPaletteAction::OpenSettings,
];

#[derive(Debug)]
pub(crate) struct CommandPaletteApp {
    pub(crate) filter_text: String,
    pub(crate) all_commands: Vec<Command>,
    pub(crate) filtered_commands: Vec<FilteredCommand>,
    pub(crate) selected_index: usize,
    pub(crate) is_open: bool,
}

impl CommandPaletteApp {
    pub(crate) fn new(all_commands: Vec<Command>) -> Self {
        let filtered_commands =
            cap_filtered_commands(initial_filtered_commands(all_commands.len()));
        Self {
            filter_text: String::new(),
            all_commands,
            filtered_commands,
            selected_index: 0,
            is_open: false,
        }
    }

    pub(crate) fn recompute_filter(&mut self) {
        self.filtered_commands =
            cap_filtered_commands(filter_commands(&self.all_commands, &self.filter_text));
    }

    pub(crate) fn draw_command_row(
        &mut self,
        ui: &mut egui::Ui,
        idx: usize,
        row: &FilteredCommand,
        keyboard_nav: &mut bool,
    ) -> Option<usize> {
        let is_selected = idx == self.selected_index;
        let orig_idx = row.command_index;
        let label = &self.all_commands[orig_idx].label;
        let shortcut_text = &self.all_commands[orig_idx].shortcut_text;
        let is_favorite = self.all_commands[orig_idx].favorite;
        let desired_size = egui::vec2(ui.available_width(), ROW_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        if response.hovered() {
            self.selected_index = idx;
        }

        let fill = if is_selected {
            PALETTE_ROW_SELECTED
        } else if response.hovered() {
            PALETTE_ROW_HOVER
        } else {
            egui::Color32::TRANSPARENT
        };

        let stroke = if is_selected {
            egui::Stroke::new(PALETTE_BORDER_WIDTH, PALETTE_ROW_SELECTED_BORDER)
        } else {
            egui::Stroke::NONE
        };

        ui.painter().rect(
            rect,
            egui::CornerRadius::same(PALETTE_ROW_RADIUS),
            fill,
            stroke,
            egui::StrokeKind::Outside,
        );

        let inner = rect.shrink2(egui::vec2(PALETTE_ROW_MARGIN_X, PALETTE_ROW_MARGIN_Y));

        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Center).with_main_justify(true),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label(highlighted_label_job(
                            label,
                            &row.label_matches,
                            is_selected,
                        ));

                        if is_favorite {
                            ui.add_space(PALETTE_FAVORITE_SPACE);
                            ui.label(
                                egui::RichText::new(PALETTE_FAVORITE_ICON)
                                    .size(PALETTE_FAVORITE_SIZE)
                                    .color(if is_selected {
                                        PALETTE_FAVORITE_SELECTED
                                    } else {
                                        PALETTE_FAVORITE_COLOR
                                    }),
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !shortcut_text.is_empty() {
                                ui.label(
                                    egui::RichText::new(shortcut_text)
                                        .size(PALETTE_SHORTCUT_SIZE)
                                        .color(if is_selected {
                                            PALETTE_SHORTCUT_SELECTED
                                        } else {
                                            PALETTE_TEXT_MUTED
                                        }),
                                );
                            }
                        });
                    });
                },
            );
        });

        let click_response = ui
            .interact(
                rect,
                ui.id().with(("command_row", orig_idx)),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        if is_selected && *keyboard_nav {
            click_response.scroll_to_me(Some(egui::Align::Center));
            *keyboard_nav = false;
        }

        if click_response.clicked() {
            self.selected_index = idx;
            return Some(orig_idx);
        }

        None
    }

    pub(crate) fn draw_settings_divider(ui: &mut egui::Ui) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(width, SETTINGS_DIVIDER_HEIGHT),
            egui::Sense::hover(),
        );
        let y = rect.center().y;
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + PALETTE_DIVIDER_MARGIN_X, y),
                egui::pos2(rect.right() - PALETTE_DIVIDER_MARGIN_X, y),
            ],
            egui::Stroke::new(PALETTE_BORDER_WIDTH, PALETTE_BORDER),
        );
    }

    pub(crate) fn draw_fixed_action_row(
        &mut self,
        ui: &mut egui::Ui,
        idx: usize,
        action: FixedPaletteAction,
        keyboard_nav: &mut bool,
    ) -> bool {
        let is_selected = idx == self.selected_index;
        let desired_size = egui::vec2(ui.available_width(), FIXED_ACTION_ROW_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        if response.hovered() {
            self.selected_index = idx;
        }

        let fill = if is_selected {
            PALETTE_ROW_SELECTED
        } else if response.hovered() {
            PALETTE_ROW_HOVER
        } else {
            egui::Color32::TRANSPARENT
        };

        let stroke = if is_selected {
            egui::Stroke::new(PALETTE_BORDER_WIDTH, PALETTE_ROW_SELECTED_BORDER)
        } else {
            egui::Stroke::NONE
        };

        ui.painter().rect(
            rect,
            egui::CornerRadius::same(PALETTE_ROW_RADIUS),
            fill,
            stroke,
            egui::StrokeKind::Outside,
        );

        let inner = rect.shrink2(egui::vec2(PALETTE_ROW_MARGIN_X, PALETTE_FIXED_ROW_MARGIN_Y));
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(action.label())
                        .size(PALETTE_ROW_LABEL_SIZE)
                        .color(if is_selected {
                            PALETTE_TEXT_SELECTED
                        } else {
                            PALETTE_TEXT_PRIMARY
                        }),
                );
            });
        });

        let click_response = ui
            .interact(rect, ui.id().with(action.id()), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        if is_selected && *keyboard_nav {
            click_response.scroll_to_me(Some(egui::Align::Center));
            *keyboard_nav = false;
        }

        if click_response.clicked() {
            self.selected_index = idx;
            return true;
        }

        false
    }
}

pub(crate) fn cap_filtered_commands(mut commands: Vec<FilteredCommand>) -> Vec<FilteredCommand> {
    commands.truncate(MAX_FILTERED_COMMANDS);
    commands
}

impl FilterableCommand for Command {
    fn label(&self) -> &str {
        &self.label
    }

    fn priority(&self) -> CommandPriority {
        self.priority
    }

    fn focus_state(&self) -> FocusState {
        self.focus_state
    }

    fn favorite(&self) -> bool {
        self.favorite
    }

    fn tags(&self) -> &[String] {
        &self.tags
    }

    fn original_order(&self) -> usize {
        self.original_order
    }
}

fn highlighted_label_job(label: &str, ranges: &[MatchRange], is_selected: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    let normal_color = if is_selected {
        PALETTE_TEXT_SELECTED
    } else {
        PALETTE_TEXT_PRIMARY
    };
    let highlight_color = if is_selected {
        PALETTE_TEXT_MATCH_SELECTED
    } else {
        PALETTE_TEXT_MATCH
    };
    let normal_format = egui::TextFormat {
        font_id: egui::FontId::proportional(PALETTE_ROW_LABEL_SIZE),
        color: normal_color,
        ..Default::default()
    };
    let highlight_format = egui::TextFormat {
        font_id: egui::FontId::proportional(PALETTE_ROW_LABEL_SIZE),
        color: highlight_color,
        ..Default::default()
    };

    let mut cursor = 0;
    for range in ranges {
        if range.start > label.len()
            || range.end > label.len()
            || range.start >= range.end
            || !label.is_char_boundary(range.start)
            || !label.is_char_boundary(range.end)
        {
            continue;
        }

        if cursor < range.start {
            job.append(&label[cursor..range.start], 0.0, normal_format.clone());
        }

        job.append(
            &label[range.start..range.end],
            0.0,
            highlight_format.clone(),
        );
        cursor = range.end;
    }

    if cursor < label.len() {
        job.append(&label[cursor..], 0.0, normal_format);
    }

    job
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixedPaletteAction {
    RefreshExtensions,
    OpenSettings,
}

impl FixedPaletteAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::RefreshExtensions => "Refresh extensions",
            Self::OpenSettings => "Open settings for Omni Palette",
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::RefreshExtensions => "fixed_refresh_extensions",
            Self::OpenSettings => "fixed_open_settings",
        }
    }
}

pub(crate) fn wrapped_selection_index(current: usize, visible_count: usize, delta: isize) -> usize {
    if visible_count == 0 {
        return 0;
    }

    let visible_count = visible_count as isize;
    let current = current as isize;
    (current + delta).rem_euclid(visible_count) as usize
}

pub(crate) fn fixed_action_for_index(
    selected_index: usize,
    visible_command_count: usize,
) -> Option<FixedPaletteAction> {
    selected_index
        .checked_sub(visible_command_count)
        .and_then(|fixed_index| FIXED_PALETTE_ACTIONS.get(fixed_index).copied())
}

#[cfg(test)]
mod tests {
    use super::{
        cap_filtered_commands, fixed_action_for_index, wrapped_selection_index, FilteredCommand,
        FixedPaletteAction, MAX_FILTERED_COMMANDS,
    };

    #[test]
    fn wrapped_selection_moves_down_from_middle() {
        assert_eq!(wrapped_selection_index(3, 8, 1), 4);
    }

    #[test]
    fn wrapped_selection_moves_up_from_middle() {
        assert_eq!(wrapped_selection_index(3, 8, -1), 2);
    }

    #[test]
    fn wrapped_selection_wraps_from_last_to_first() {
        assert_eq!(wrapped_selection_index(7, 8, 1), 0);
    }

    #[test]
    fn wrapped_selection_wraps_from_first_to_last() {
        assert_eq!(wrapped_selection_index(0, 8, -1), 7);
    }

    #[test]
    fn wrapped_selection_is_zero_when_no_rows_are_visible() {
        assert_eq!(wrapped_selection_index(4, 0, 1), 0);
    }

    #[test]
    fn fixed_action_after_last_command_is_refresh_extensions() {
        assert_eq!(
            fixed_action_for_index(5, 5),
            Some(FixedPaletteAction::RefreshExtensions)
        );
    }

    #[test]
    fn fixed_action_after_refresh_is_open_settings() {
        assert_eq!(
            fixed_action_for_index(6, 5),
            Some(FixedPaletteAction::OpenSettings)
        );
    }

    #[test]
    fn fixed_action_ignores_command_rows() {
        assert_eq!(fixed_action_for_index(4, 5), None);
    }

    #[test]
    fn selection_moves_from_last_command_to_refresh_extensions() {
        assert_eq!(wrapped_selection_index(4, 7, 1), 5);
        assert_eq!(
            fixed_action_for_index(wrapped_selection_index(4, 7, 1), 5),
            Some(FixedPaletteAction::RefreshExtensions)
        );
    }

    #[test]
    fn selection_moves_from_refresh_to_open_settings() {
        assert_eq!(wrapped_selection_index(5, 7, 1), 6);
        assert_eq!(
            fixed_action_for_index(wrapped_selection_index(5, 7, 1), 5),
            Some(FixedPaletteAction::OpenSettings)
        );
    }

    #[test]
    fn selection_wraps_from_open_settings_to_first_row() {
        assert_eq!(wrapped_selection_index(6, 7, 1), 0);
    }

    #[test]
    fn zero_command_results_wrap_between_fixed_actions() {
        assert_eq!(
            fixed_action_for_index(wrapped_selection_index(0, 2, 1), 0),
            Some(FixedPaletteAction::OpenSettings)
        );
        assert_eq!(
            fixed_action_for_index(wrapped_selection_index(1, 2, 1), 0),
            Some(FixedPaletteAction::RefreshExtensions)
        );
    }

    #[test]
    fn fixed_actions_start_after_all_rendered_commands_not_visible_cap() {
        assert_eq!(fixed_action_for_index(9, 18), None);
        assert_eq!(fixed_action_for_index(10, 18), None);
        assert_eq!(fixed_action_for_index(17, 18), None);
        assert_eq!(
            fixed_action_for_index(18, 18),
            Some(FixedPaletteAction::RefreshExtensions)
        );
        assert_eq!(
            fixed_action_for_index(19, 18),
            Some(FixedPaletteAction::OpenSettings)
        );
    }

    #[test]
    fn filtered_commands_are_capped_to_eighteen_items() {
        let rows: Vec<FilteredCommand> = (0..25)
            .map(|command_index| FilteredCommand {
                command_index,
                score: 0,
                label_matches: Vec::new(),
                is_prefix: false,
                span: 0,
            })
            .collect();

        let capped = cap_filtered_commands(rows);

        assert_eq!(capped.len(), MAX_FILTERED_COMMANDS);
        assert_eq!(capped.last().map(|row| row.command_index), Some(17));
    }
}
