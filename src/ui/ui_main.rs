use crate::core::search::get_score;
use eframe::egui;
use std::fmt;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

const PALETTE_WIDTH: f32 = 720.0;
const MAX_VISIBLE_ROWS: usize = 8;
const ROW_HEIGHT: f32 = 40.0;

const BG: egui::Color32 = egui::Color32::from_rgb(30, 30, 30);
const CARD_BG: egui::Color32 = egui::Color32::from_rgb(37, 37, 38);
const BORDER: egui::Color32 = egui::Color32::from_rgb(69, 69, 69);
const SEARCH_BG: egui::Color32 = egui::Color32::from_rgb(30, 30, 30);
const SEARCH_BORDER: egui::Color32 = egui::Color32::from_rgb(82, 82, 82);
const ROW_HOVER: egui::Color32 = egui::Color32::from_rgb(43, 43, 43);
const ROW_SELECTED: egui::Color32 = egui::Color32::from_rgb(9, 71, 113);
const ROW_SELECTED_BORDER: egui::Color32 = egui::Color32::from_rgb(55, 148, 255);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(204, 204, 204);
const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(150, 150, 150);
const TEXT_SELECTED: egui::Color32 = egui::Color32::WHITE;
const TEXTBOX_TEXT: egui::Color32 = egui::Color32::from_rgb(230, 230, 230);
const TEXTBOX_HINT: egui::Color32 = egui::Color32::from_rgb(120, 120, 120);
const TEXTBOX_CURSOR: egui::Color32 = egui::Color32::from_rgb(0, 122, 204);

pub struct Command {
    pub label: String,
    pub action: Box<dyn Fn() + Send + Sync>,
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("label", &self.label)
            .field("action", &"<function>")
            .finish()
    }
}

#[derive(Debug)]
pub struct CommandPaletteApp {
    pub filter_text: String,
    pub all_commands: Vec<Command>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub is_open: bool,
}

impl CommandPaletteApp {
    fn new(all_commands: Vec<Command>) -> Self {
        let filtered_indices = (0..all_commands.len()).collect();
        Self {
            filter_text: String::new(),
            all_commands,
            filtered_indices,
            selected_index: 0,
            is_open: false,
        }
    }

    fn recompute_filter(&mut self) {
        if self.filter_text.is_empty() {
            self.filtered_indices = (0..self.all_commands.len()).collect();
            return;
        }

        let mut scored: Vec<(usize, i32)> = self
            .all_commands
            .iter()
            .enumerate()
            .filter_map(|(i, cmd)| {
                let result = get_score(&cmd.label, &self.filter_text);
                (result.score > 0).then_some((i, result.score))
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        self.filtered_indices = scored.into_iter().map(|(i, _)| i).collect();
    }
}

#[derive(Debug)]
pub enum UiSignal {
    Show(Vec<Command>),
    Hide,
}

#[derive(Debug)]
pub enum UiEvent {
    Closed,
    ActionExecuted,
}

struct App {
    receiver: Receiver<UiSignal>,
    palette: CommandPaletteApp,
    event_tx: Sender<UiEvent>,
    had_focus_since_open: bool,
    needs_text_focus: bool,
}

impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        receiver: Receiver<UiSignal>,
        event_tx: Sender<UiEvent>,
    ) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::TRANSPARENT;
        visuals.window_fill = egui::Color32::TRANSPARENT;
        visuals.extreme_bg_color = SEARCH_BG;
        visuals.faint_bg_color = CARD_BG;
        visuals.override_text_color = Some(TEXT_PRIMARY);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
        visuals.widgets.inactive.bg_fill = SEARCH_BG;
        visuals.widgets.inactive.weak_bg_fill = SEARCH_BG;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, SEARCH_BORDER);
        visuals.widgets.inactive.fg_stroke.color = TEXT_PRIMARY;
        visuals.widgets.hovered.bg_fill = ROW_HOVER;
        visuals.widgets.hovered.weak_bg_fill = ROW_HOVER;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, SEARCH_BORDER);
        visuals.widgets.hovered.fg_stroke.color = TEXT_PRIMARY;
        visuals.widgets.active.bg_fill = ROW_SELECTED;
        visuals.widgets.active.weak_bg_fill = ROW_SELECTED;
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ROW_SELECTED_BORDER);
        visuals.widgets.active.fg_stroke.color = TEXT_SELECTED;
        visuals.widgets.open.bg_fill = ROW_SELECTED;
        visuals.widgets.open.weak_bg_fill = ROW_SELECTED;
        visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, ROW_SELECTED_BORDER);
        visuals.widgets.open.fg_stroke.color = TEXT_SELECTED;
        cc.egui_ctx.set_visuals(visuals);

        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(0.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(0);
        cc.egui_ctx.set_style(style);

        Self {
            palette: CommandPaletteApp::new(vec![]),
            receiver,
            event_tx,
            had_focus_since_open: false,
            needs_text_focus: false,
        }
    }

    fn show(&mut self, ctx: &egui::Context, commands: Vec<Command>) {
        self.palette.all_commands = commands;
        self.palette.filter_text.clear();
        self.palette.selected_index = 0;
        self.palette.recompute_filter();
        self.palette.is_open = true;
        self.had_focus_since_open = false;
        self.needs_text_focus = true;

        self.sync_viewport(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn hide(&mut self, ctx: &egui::Context) {
        if !self.palette.is_open {
            return;
        }

        self.palette.is_open = false;
        self.had_focus_since_open = false;
        self.needs_text_focus = false;

        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        let _ = self.event_tx.send(UiEvent::Closed);
    }

    fn desired_window_size(&self) -> egui::Vec2 {
        let visible_count = self.palette.filtered_indices.len().min(MAX_VISIBLE_ROWS);
        let list_height = if visible_count == 0 {
            44.0
        } else {
            visible_count as f32 * ROW_HEIGHT
        };

        egui::vec2(PALETTE_WIDTH, 16.0 + 52.0 + 8.0 + list_height + 16.0)
    }

    fn sync_viewport(&self, ctx: &egui::Context) {
        let size = self.desired_window_size();
        let monitor_size = ctx
            .input(|i| i.viewport().monitor_size)
            .unwrap_or(egui::vec2(1920.0, 1080.0));

        let x = (monitor_size.x - size.x) / 2.0;
        let y = monitor_size.y * 0.10;

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
    }

    fn split_label<'a>(&self, label: &'a str) -> (&'a str, &'a str) {
        if let Some((app, action)) = label.split_once(": ") {
            (app, action)
        } else {
            ("", label)
        }
    }

    fn execute_selected(&mut self, ctx: &egui::Context) {
        if let Some(&orig_idx) = self
            .palette
            .filtered_indices
            .get(self.palette.selected_index)
        {
            (self.palette.all_commands[orig_idx].action)();
            let _ = self.event_tx.send(UiEvent::ActionExecuted);
            self.hide(ctx);
        }
    }

    fn draw_command_row(
        &mut self,
        ui: &mut egui::Ui,
        idx: usize,
        orig_idx: usize,
    ) -> Option<usize> {
        let is_selected = idx == self.palette.selected_index;
        let label = &self.palette.all_commands[orig_idx].label;
        let (app, action) = self.split_label(label);

        let desired_size = egui::vec2(ui.available_width(), ROW_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if response.hovered() {
            self.palette.selected_index = idx;
        }

        let fill = if is_selected {
            ROW_SELECTED
        } else if response.hovered() {
            ROW_HOVER
        } else {
            egui::Color32::TRANSPARENT
        };

        let stroke = if is_selected {
            egui::Stroke::new(1.0, ROW_SELECTED_BORDER)
        } else {
            egui::Stroke::NONE
        };

        ui.painter().rect(
            rect,
            egui::CornerRadius::same(6),
            fill,
            stroke,
            egui::StrokeKind::Outside,
        );

        let inner = rect.shrink2(egui::vec2(12.0, 8.0));

        ui.allocate_ui_at_rect(inner, |ui| {
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Center).with_main_justify(true),
                |ui| {
                    ui.horizontal(|ui| {
                        if !app.is_empty() {
                            ui.label(egui::RichText::new(format!("{app}:")).size(14.5).color(
                                if is_selected {
                                    egui::Color32::from_rgb(220, 235, 255)
                                } else {
                                    TEXT_MUTED
                                },
                            ));
                            ui.add_space(8.0);
                        }

                        ui.label(
                            egui::RichText::new(action)
                                .size(15.5)
                                .color(if is_selected {
                                    TEXT_SELECTED
                                } else {
                                    TEXT_PRIMARY
                                }),
                        );
                    });
                },
            );
        });

        if is_selected {
            response.scroll_to_me(Some(egui::Align::Center));
        }

        if response.clicked() {
            self.palette.selected_index = idx;
            return Some(orig_idx);
        }

        None
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(16));

        while let Ok(sig) = self.receiver.try_recv() {
            match sig {
                UiSignal::Show(commands) => self.show(ctx, commands),
                UiSignal::Hide => self.hide(ctx),
            }
        }

        if !self.palette.is_open {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }

        self.sync_viewport(ctx);

        let is_focused = ctx.input(|i| i.focused);
        if is_focused {
            self.had_focus_since_open = true;
        } else if self.had_focus_since_open {
            self.hide(ctx);
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.hide(ctx);
            return;
        }

        let visible_count = self.palette.filtered_indices.len().min(MAX_VISIBLE_ROWS);

        if visible_count > 0 {
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                self.palette.selected_index = (self.palette.selected_index + 1) % visible_count;
            }

            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                self.palette.selected_index =
                    (self.palette.selected_index + visible_count - 1) % visible_count;
            }

            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.execute_selected(ctx);
                return;
            }
        }

        egui::Area::new(egui::Id::new("command_palette_area"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 0.0))
            .movable(false)
            .interactable(true)
            .show(ctx, |ui| {
                ui.set_width(PALETTE_WIDTH);

                egui::Frame::new()
                    .fill(CARD_BG)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .fill(SEARCH_BG)
                            .stroke(egui::Stroke::new(1.0, SEARCH_BORDER))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin {
                                left: 12,
                                right: 12,
                                top: 10,
                                bottom: 10,
                            })
                            .show(ui, |ui| {
                                ui.scope(|ui| {
                                    ui.visuals_mut().override_text_color = Some(TEXTBOX_TEXT);
                                    ui.visuals_mut().selection.bg_fill = ROW_SELECTED;
                                    ui.visuals_mut().selection.stroke =
                                        egui::Stroke::new(1.0, ROW_SELECTED_BORDER);
                                    ui.visuals_mut().text_cursor.stroke =
                                        egui::Stroke::new(2.0, TEXTBOX_CURSOR);

                                    ui.horizontal(|ui| {
                                        ui.add_space(2.0);
                                        ui.label(
                                            egui::RichText::new(">").size(18.0).color(TEXTBOX_HINT),
                                        );
                                        ui.add_space(6.0);

                                        let resp = ui.add_sized(
                                            [ui.available_width(), 28.0],
                                            egui::TextEdit::singleline(
                                                &mut self.palette.filter_text,
                                            )
                                            .hint_text(
                                                egui::RichText::new("Type a command")
                                                    .color(TEXTBOX_HINT)
                                                    .size(16.0),
                                            )
                                            .font(egui::TextStyle::Heading)
                                            .frame(egui::Frame::NONE),
                                        );

                                        if self.needs_text_focus {
                                            resp.request_focus();
                                            self.needs_text_focus = false;
                                        }

                                        if resp.changed() {
                                            self.palette.selected_index = 0;
                                            self.palette.recompute_filter();
                                            self.sync_viewport(ui.ctx());
                                        }
                                    });
                                });
                            });

                        ui.add_space(8.0);

                        if self.palette.filtered_indices.is_empty() {
                            egui::Frame::new()
                                .fill(BG)
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(egui::Margin::same(12))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new("No matching commands")
                                            .size(14.5)
                                            .color(TEXT_MUTED),
                                    );
                                });
                            return;
                        }

                        let max_height = MAX_VISIBLE_ROWS as f32 * ROW_HEIGHT;

                        egui::ScrollArea::vertical()
                            .max_height(max_height)
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                let mut clicked_action: Option<usize> = None;

                                let indices: Vec<(usize, usize)> = self
                                    .palette
                                    .filtered_indices
                                    .iter()
                                    .take(MAX_VISIBLE_ROWS)
                                    .copied()
                                    .enumerate()
                                    .collect();

                                for (idx, orig_idx) in indices {
                                    if let Some(clicked_idx) =
                                        self.draw_command_row(ui, idx, orig_idx)
                                    {
                                        clicked_action = Some(clicked_idx);
                                    }
                                }

                                if let Some(orig_idx) = clicked_action {
                                    (self.palette.all_commands[orig_idx].action)();
                                    let _ = self.event_tx.send(UiEvent::ActionExecuted);
                                    self.hide(ui.ctx());
                                }
                            });
                    });
            });
    }

    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
}

pub fn ui_main(receiver: Receiver<UiSignal>, event_tx: Sender<UiEvent>) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([PALETTE_WIDTH, 260.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_visible(false)
            .with_active(true),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Command Palette",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, receiver, event_tx)))),
    );
}
