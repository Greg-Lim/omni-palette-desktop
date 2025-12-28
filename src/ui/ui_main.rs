use eframe::egui;
use std::fmt;
use std::sync::mpsc::{self, Receiver};

/// Represents a single command entry shown in the palette UI
pub struct Command {
    pub label: String,
    // Placeholder for the command action; wiring happens elsewhere.
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

/// Core UI state for the command palette.
#[derive(Debug)]
pub struct CommandPaletteApp {
    /// What the user is currently typing.
    pub filter_text: String,
    /// All possible commands available in the system.
    pub all_commands: Vec<Command>,
    /// Indices of commands from `all_commands` that match `filter_text`.
    pub filtered_indices: Vec<usize>,
    /// Which item in the filtered list is currently highlighted (via arrow keys).
    pub selected_index: usize,
    /// Whether the palette is currently visible.
    pub is_open: bool,
}

impl CommandPaletteApp {
    fn new(all_commands: Vec<Command>) -> Self {
        let mut s = Self {
            filter_text: String::new(),
            all_commands,
            filtered_indices: Vec::new(),
            selected_index: 0,
            is_open: true, // Start visible
        };
        s
    }
}

#[derive(Debug)]
pub enum UiSignal {
    ToggleVisibility,
}

struct App {
    // UI
    receiver: Receiver<UiSignal>, // Receives signals from hotkey thread

    // State
    palette: CommandPaletteApp,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>, receiver: Receiver<UiSignal>) -> Self {
        // Example commands (replace with real actions when wiring platform)
        let demo_commands = vec![Command {
            label: "Quit".to_string(),
            action: Box::new(|| println!("Action: Quit")),
        }];

        Self {
            palette: CommandPaletteApp::new(demo_commands),
            receiver,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // CRITICAL: Always request repaint to ensure update loop continues even when hidden
        ctx.request_repaint();

        // CRITICAL: Always process incoming UI signals regardless of visibility
        let mut signal_received = false;
        while let Ok(sig) = self.receiver.try_recv() {
            signal_received = true;
            dbg!(sig);
            match sig {
                UiSignal::ToggleVisibility => {
                    let old_state = self.palette.is_open;
                    self.palette.is_open = !self.palette.is_open;
                    println!(
                        "Toggle: {} → {} (palette.is_open)",
                        old_state, self.palette.is_open
                    );

                    if self.palette.is_open {
                        // Show and focus the window
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        self.palette.filter_text.clear();
                        self.palette.selected_index = 0;
                        println!("Window should now be visible and focused");
                    } else {
                        // Hide window by making it minimized and off-screen
                        // This keeps it hidden from Alt+Tab but allows update loop to continue
                        // this is very hacky
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                            -2000.0, -2000.0,
                        )));
                        println!("Window minimized and moved off-screen (hidden from Alt+Tab)");
                    }
                }
            }
        }

        // Only do UI positioning and keyboard handling when visible
        if self.palette.is_open {
            // Position window once
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                (ctx.input(|i| i.viewport().monitor_size.map(|m| m.x))
                    .unwrap_or(1920.0)
                    - 600.0)
                    / 2.0,
                ctx.input(|i| i.viewport().monitor_size.map(|m| m.y))
                    .unwrap_or(1080.0)
                    * 0.15,
            )));

            // Hide on Escape (same behavior as Ctrl+Shift+P)
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.palette.is_open = false;
                // Hide window by making it minimized and off-screen
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                    -2000.0, -2000.0,
                )));
                println!(
                    "Escape pressed: Window minimized and moved off-screen (hidden from Alt+Tab)"
                );
            }

            // Keyboard navigation (only when visible)
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
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                            -2000.0, -2000.0,
                        )));
                    }
                }
            }

            // Dynamically adjust viewport height based on results
            let desired_height = 44.0 + (visible_count as f32) * 28.0 + 12.0; // input + rows + padding
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                600.0,
                desired_height.max(80.0),
            )));
        }

        // The UI (render only when visible)
        if self.palette.is_open {
            egui::CentralPanel::default().show(ctx, |ui| {
                let frame = egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 230))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)))
                    .rounding(6.0);

                frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    // Search box
                    let resp = ui.add_sized(
                        [ui.available_width(), 34.0],
                        egui::TextEdit::singleline(&mut self.palette.filter_text)
                            .hint_text("Search commands..."),
                    );
                    resp.request_focus();
                    if resp.changed() {
                        self.palette.selected_index = 0;
                    }

                    ui.add_space(6.0);

                    // Results list (limited)
                    for (idx, &orig_idx) in self.palette.filtered_indices.iter().take(8).enumerate()
                    {
                        let is_selected = idx == self.palette.selected_index;
                        let label = &self.palette.all_commands[orig_idx].label;
                        let row = ui.selectable_label(is_selected, label);
                        if row.clicked() {
                            self.palette.selected_index = idx;
                            (self.palette.all_commands[orig_idx].action)();
                            self.palette.is_open = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                                egui::pos2(-2000.0, -2000.0),
                            ));
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
}

pub fn ui_main(receiver: Receiver<UiSignal>) {
    let width = 600.0;
    let height = 180.0;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([width, height])
            .with_decorations(true) // Add decorations to make it more stable
            .with_transparent(false) // Disable transparency to avoid issues
            .with_always_on_top()
            .with_resizable(false)
            .with_visible(true) // Start visible for testing
            .with_active(true),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Command Palette",
        options,
        Box::new(move |_cc| Ok(Box::new(App::new(_cc, receiver)))),
    );
}
