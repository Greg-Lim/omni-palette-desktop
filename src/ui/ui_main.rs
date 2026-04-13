use crate::core::search::get_score;
use eframe::egui;
use std::fmt;
use std::sync::mpsc::Receiver;

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
            is_open: true,
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
                if result.score > 0 {
                    Some((i, result.score))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        self.filtered_indices = scored.into_iter().map(|(i, _)| i).collect();
    }
}

#[derive(Debug)]
pub enum UiSignal {
    ToggleVisibility,
}

struct App {
    receiver: Receiver<UiSignal>,
    palette: CommandPaletteApp,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>, receiver: Receiver<UiSignal>) -> Self {
        let demo_commands = vec![Command {
            label: "Quit".to_string(),
            action: Box::new(|| println!("Action: Quit")),
        }];

        Self {
            palette: CommandPaletteApp::new(demo_commands),
            receiver,
        }
    }

    fn hide(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        while let Ok(sig) = self.receiver.try_recv() {
            match sig {
                UiSignal::ToggleVisibility => {
                    self.palette.is_open = !self.palette.is_open;

                    if self.palette.is_open {
                        self.palette.filter_text.clear();
                        self.palette.selected_index = 0;
                        self.palette.recompute_filter();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    } else {
                        self.hide(ctx);
                    }
                }
            }
        }

        if self.palette.is_open {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                (ctx.input(|i| i.viewport().monitor_size.map(|m| m.x))
                    .unwrap_or(1920.0)
                    - 600.0)
                    / 2.0,
                ctx.input(|i| i.viewport().monitor_size.map(|m| m.y))
                    .unwrap_or(1080.0)
                    * 0.15,
            )));

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.palette.is_open = false;
                self.hide(ctx);
            }

            let visible_count = self.palette.filtered_indices.len().min(8);
            if visible_count > 0 {
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    if self.palette.selected_index + 1 < visible_count {
                        self.palette.selected_index += 1;
                    }
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    if self.palette.selected_index > 0 {
                        self.palette.selected_index -= 1;
                    }
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some(&orig_idx) = self
                        .palette
                        .filtered_indices
                        .get(self.palette.selected_index)
                    {
                        (self.palette.all_commands[orig_idx].action)();
                        self.palette.is_open = false;
                        self.hide(ctx);
                    }
                }
            }

            let desired_height = 44.0 + (visible_count as f32) * 28.0 + 12.0;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                600.0,
                desired_height.max(80.0),
            )));
        }

        if self.palette.is_open {
            egui::CentralPanel::default().show(ctx, |ui| {
                let frame = egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 230))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)))
                    .rounding(6.0);

                frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    let resp = ui.add_sized(
                        [ui.available_width(), 34.0],
                        egui::TextEdit::singleline(&mut self.palette.filter_text)
                            .hint_text("Search commands..."),
                    );
                    resp.request_focus();
                    if resp.changed() {
                        self.palette.selected_index = 0;
                        self.palette.recompute_filter();
                    }

                    ui.add_space(6.0);

                    for (idx, &orig_idx) in self.palette.filtered_indices.iter().take(8).enumerate()
                    {
                        let is_selected = idx == self.palette.selected_index;
                        let label = &self.palette.all_commands[orig_idx].label;
                        let row = ui.selectable_label(is_selected, label);
                        if row.clicked() {
                            self.palette.selected_index = idx;
                            (self.palette.all_commands[orig_idx].action)();
                            self.palette.is_open = false;
                            self.hide(ctx);
                        }
                    }

                    if self.palette.filtered_indices.is_empty() {
                        ui.label(
                            egui::RichText::new("No commands")
                                .italics()
                                .color(egui::Color32::GRAY),
                        );
                    }
                });
            });
        }
    }

    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}
}

pub fn ui_main(receiver: Receiver<UiSignal>) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 180.0])
            .with_decorations(true)
            .with_transparent(false)
            .with_always_on_top()
            .with_resizable(false)
            .with_visible(true)
            .with_active(true),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Command Palette",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, receiver)))),
    );
}
