use crate::config::runtime::{CommandBehavior, RuntimeConfig};
use crate::core::command_filter::FilteredCommand;
use crate::core::extensions::catalog::{CatalogEntry, ExtensionCatalog};
use crate::core::extensions::install::{BundledExtension, InstalledState};
use crate::domain::action::{CommandPriority, FocusState};
use crate::platform::ui_support::{
    focus_window_token, foreground_window_token, PlatformUiAction, PlatformUiRuntime,
    PlatformWindowToken,
};
pub use crate::ui::guide::GuideHint;
use crate::ui::guide::{close_guide_viewport, show_guide_viewport, ActiveGuide, GUIDE_DURATION};
use crate::ui::palette::{
    fixed_action_for_index, wrapped_selection_index, CommandPaletteApp, FixedPaletteAction,
    ESTIMATED_VISIBLE_ROW_HEIGHT, FIXED_ACTION_ROW_HEIGHT, FIXED_PALETTE_ACTIONS,
    MAX_VISIBLE_COMMAND_ROWS, PALETTE_BORDER, PALETTE_BORDER_WIDTH, PALETTE_CARD_BG,
    PALETTE_CURSOR_WIDTH, PALETTE_EMPTY_ROW_MARGIN, PALETTE_EMPTY_ROW_RADIUS,
    PALETTE_EMPTY_TEXT_SIZE, PALETTE_FRAME_MARGIN, PALETTE_FRAME_RADIUS, PALETTE_RESULTS_TOP_SPACE,
    PALETTE_ROW_HOVER, PALETTE_ROW_SELECTED, PALETTE_ROW_SELECTED_BORDER, PALETTE_SEARCH_BG,
    PALETTE_SEARCH_BORDER, PALETTE_SEARCH_HEIGHT, PALETTE_SEARCH_MARGIN_X, PALETTE_SEARCH_MARGIN_Y,
    PALETTE_SEARCH_PROMPT_LEFT_SPACE, PALETTE_SEARCH_PROMPT_RIGHT_SPACE,
    PALETTE_SEARCH_PROMPT_SIZE, PALETTE_SEARCH_RADIUS, PALETTE_TEXTBOX_CURSOR,
    PALETTE_TEXTBOX_HINT, PALETTE_TEXTBOX_TEXT, PALETTE_TEXT_MUTED, PALETTE_TEXT_PRIMARY,
    PALETTE_TEXT_SELECTED, PALETTE_WIDTH, PALETTE_WINDOW_BG, ROW_HEIGHT, SETTINGS_DIVIDER_HEIGHT,
};
use crate::ui::settings::{show_settings_viewport, SettingsBootstrap, SettingsState};
use eframe::egui;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub struct Command {
    pub label: String,
    pub shortcut_text: String,
    pub guide_hint: Option<GuideHint>,
    pub priority: CommandPriority,
    pub focus_state: FocusState,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub original_order: usize,
    pub action: Box<dyn Fn() + Send + Sync>,
}

#[derive(Debug, Clone, Copy)]
pub struct PaletteWorkArea {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl PaletteWorkArea {
    pub fn from_ltrb(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left: left as f32,
            top: top as f32,
            right: right as f32,
            bottom: bottom as f32,
        }
    }

    pub(crate) fn width(self) -> f32 {
        self.right - self.left
    }

    pub(crate) fn height(self) -> f32 {
        self.bottom - self.top
    }

    pub(crate) fn to_points(self, native_pixels_per_point: f32) -> Self {
        Self {
            left: self.left / native_pixels_per_point,
            top: self.top / native_pixels_per_point,
            right: self.right / native_pixels_per_point,
            bottom: self.bottom / native_pixels_per_point,
        }
    }
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("label", &self.label)
            .field("shortcut_text", &self.shortcut_text)
            .field("guide_hint", &self.guide_hint)
            .field("priority", &self.priority)
            .field("focus_state", &self.focus_state)
            .field("favorite", &self.favorite)
            .field("tags", &self.tags)
            .field("original_order", &self.original_order)
            .field("action", &"<function>")
            .finish()
    }
}

#[derive(Debug)]
pub struct InstalledExtensionsUpdate {
    pub state: InstalledState,
    pub message: String,
}

#[derive(Debug)]
pub enum UiSignal {
    Show {
        commands: Vec<Command>,
        work_area: Option<PaletteWorkArea>,
        command_behavior: CommandBehavior,
        activation_hint: String,
    },
    Hide,
    RunGuidedAction,
    CancelGuide,
    RuntimeConfigSaved {
        config: RuntimeConfig,
        result: Result<String, String>,
    },
    CatalogRefreshed(Result<ExtensionCatalog, String>),
    InstalledExtensionsUpdated(Result<InstalledExtensionsUpdate, String>),
    ReloadExtensionsFinished(Result<String, String>),
    Quit,
}

#[derive(Debug)]
pub enum UiEvent {
    Closed,
    ActionExecuted,
    GuideStarted {
        shortcut: Option<crate::domain::hotkey::KeyboardShortcut>,
    },
    GuideEnded,
    OpenPaletteRequested,
    SaveRuntimeConfigRequested(RuntimeConfig),
    RefreshCatalogRequested(crate::config::runtime::GitHubExtensionSource),
    InstallExtensionRequested {
        source: crate::config::runtime::GitHubExtensionSource,
        entry: CatalogEntry,
        installed_version: Option<String>,
    },
    UninstallExtensionRequested {
        extension_id: String,
        source_id: String,
        display_name: String,
    },
    SetExtensionEnabledRequested {
        extension_id: String,
        source_id: String,
        display_name: String,
        enabled: bool,
    },
    SetBundledExtensionEnabledRequested {
        extension: BundledExtension,
        enabled: bool,
    },
    ReloadExtensionsRequested,
    QuitRequested,
}

pub type SharedUiContext = Arc<OnceLock<egui::Context>>;
pub type SharedUiVisibility = Arc<AtomicBool>;

struct App {
    receiver: Receiver<UiSignal>,
    palette: CommandPaletteApp,
    settings: Arc<Mutex<SettingsState>>,
    event_tx: Sender<UiEvent>,
    had_focus_since_open: bool,
    needs_text_focus: bool,
    keyboard_nav: bool,
    work_area: Option<PaletteWorkArea>,
    command_behavior: CommandBehavior,
    activation_hint: String,
    guide: Option<ActiveGuide>,
    visibility: SharedUiVisibility,
    platform_ui: PlatformUiRuntime,
}

impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        receiver: Receiver<UiSignal>,
        event_tx: Sender<UiEvent>,
        shared_context: SharedUiContext,
        visibility: SharedUiVisibility,
        settings_bootstrap: SettingsBootstrap,
    ) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::TRANSPARENT;
        visuals.window_fill = egui::Color32::TRANSPARENT;
        visuals.extreme_bg_color = PALETTE_SEARCH_BG;
        visuals.faint_bg_color = PALETTE_CARD_BG;
        visuals.override_text_color = Some(PALETTE_TEXT_PRIMARY);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
        visuals.widgets.inactive.bg_fill = PALETTE_SEARCH_BG;
        visuals.widgets.inactive.weak_bg_fill = PALETTE_SEARCH_BG;
        visuals.widgets.inactive.bg_stroke =
            egui::Stroke::new(PALETTE_BORDER_WIDTH, PALETTE_SEARCH_BORDER);
        visuals.widgets.inactive.fg_stroke.color = PALETTE_TEXT_PRIMARY;
        visuals.widgets.hovered.bg_fill = PALETTE_ROW_HOVER;
        visuals.widgets.hovered.weak_bg_fill = PALETTE_ROW_HOVER;
        visuals.widgets.hovered.bg_stroke =
            egui::Stroke::new(PALETTE_BORDER_WIDTH, PALETTE_SEARCH_BORDER);
        visuals.widgets.hovered.fg_stroke.color = PALETTE_TEXT_PRIMARY;
        visuals.widgets.active.bg_fill = PALETTE_ROW_SELECTED;
        visuals.widgets.active.weak_bg_fill = PALETTE_ROW_SELECTED;
        visuals.widgets.active.bg_stroke =
            egui::Stroke::new(PALETTE_BORDER_WIDTH, PALETTE_ROW_SELECTED_BORDER);
        visuals.widgets.active.fg_stroke.color = PALETTE_TEXT_SELECTED;
        visuals.widgets.open.bg_fill = PALETTE_ROW_SELECTED;
        visuals.widgets.open.weak_bg_fill = PALETTE_ROW_SELECTED;
        visuals.widgets.open.bg_stroke =
            egui::Stroke::new(PALETTE_BORDER_WIDTH, PALETTE_ROW_SELECTED_BORDER);
        visuals.widgets.open.fg_stroke.color = PALETTE_TEXT_SELECTED;
        cc.egui_ctx.set_visuals(visuals);

        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.spacing.item_spacing = egui::vec2(0.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(0);
        cc.egui_ctx.set_global_style(style);
        let _ = shared_context.set(cc.egui_ctx.clone());
        visibility.store(false, Ordering::Relaxed);
        let command_behavior = settings_bootstrap.config.command_behavior;
        let activation_hint = settings_bootstrap.config.activation.to_string();
        let settings = Arc::new(Mutex::new(SettingsState::new(settings_bootstrap)));
        let platform_ui = PlatformUiRuntime::new(cc, &cc.egui_ctx);

        Self {
            palette: CommandPaletteApp::new(vec![]),
            settings,
            receiver,
            event_tx,
            had_focus_since_open: false,
            needs_text_focus: false,
            keyboard_nav: false,
            work_area: None,
            command_behavior,
            activation_hint,
            guide: None,
            visibility,
            platform_ui,
        }
    }

    fn show(
        &mut self,
        ctx: &egui::Context,
        commands: Vec<Command>,
        work_area: Option<PaletteWorkArea>,
        command_behavior: CommandBehavior,
        activation_hint: String,
    ) {
        self.end_guide(ctx, true);
        self.palette.all_commands = commands;
        self.palette.filter_text.clear();
        self.palette.selected_index = 0;
        self.palette.recompute_filter();
        self.palette.is_open = true;
        self.work_area = work_area;
        self.command_behavior = command_behavior;
        self.activation_hint = activation_hint;
        self.had_focus_since_open = false;
        self.needs_text_focus = true;
        self.visibility.store(true, Ordering::Relaxed);
        log::debug!(
            "Showing palette: commands={}, work_area={:?}",
            self.palette.all_commands.len(),
            self.work_area
        );

        self.sync_viewport(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn hide(&mut self, ctx: &egui::Context, reason: &str) {
        if !self.palette.is_open {
            return;
        }

        log::debug!("Hiding palette: reason={reason}");

        self.palette.is_open = false;
        self.work_area = None;
        self.had_focus_since_open = false;
        self.needs_text_focus = false;
        self.visibility.store(false, Ordering::Relaxed);

        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        let _ = self.event_tx.send(UiEvent::Closed);
    }

    fn settings_open(&self) -> bool {
        self.settings
            .lock()
            .map(|settings| settings.open)
            .unwrap_or(false)
    }

    fn handle_platform_action(&self, ctx: &egui::Context, action: PlatformUiAction) {
        apply_platform_action(ctx, &self.settings, &self.event_tx, action);
    }

    fn handle_signal(&mut self, ctx: &egui::Context, signal: UiSignal) {
        match signal {
            UiSignal::Show {
                commands,
                work_area,
                command_behavior,
                activation_hint,
            } => self.show(ctx, commands, work_area, command_behavior, activation_hint),
            UiSignal::Hide => self.hide(ctx, "signal"),
            UiSignal::RunGuidedAction => self.run_guided_action(ctx),
            UiSignal::CancelGuide => self.end_guide(ctx, true),
            UiSignal::RuntimeConfigSaved { config, result } => {
                if let Ok(mut settings) = self.settings.lock() {
                    settings.config_saved(config, result);
                }
            }
            UiSignal::CatalogRefreshed(result) => {
                if let Ok(mut settings) = self.settings.lock() {
                    settings.catalog_refreshed(result);
                }
            }
            UiSignal::InstalledExtensionsUpdated(result) => {
                if let Ok(mut settings) = self.settings.lock() {
                    settings.installed_extensions_updated(result);
                }
            }
            UiSignal::ReloadExtensionsFinished(result) => {
                if let Ok(mut settings) = self.settings.lock() {
                    settings.reload_finished(result);
                }
            }
            UiSignal::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn desired_window_size(&self) -> egui::Vec2 {
        egui::vec2(
            PALETTE_WIDTH,
            16.0 + 60.0 + 8.0 + self.current_list_height() + 16.0,
        )
    }

    fn sync_viewport(&self, ctx: &egui::Context) {
        let size = self.desired_window_size();
        let native_pixels_per_point = ctx
            .input(|i| i.viewport().native_pixels_per_point)
            .unwrap_or(1.0)
            .max(1.0);

        let (x, y) = if let Some(work_area) = self.work_area {
            let work_area = work_area.to_points(native_pixels_per_point);
            (
                work_area.left + ((work_area.width() - size.x) / 2.0).max(0.0),
                work_area.top + (work_area.height() * 0.10),
            )
        } else {
            let monitor_size = ctx
                .input(|i| i.viewport().monitor_size)
                .unwrap_or(egui::vec2(1920.0, 1080.0));
            ((monitor_size.x - size.x) / 2.0, monitor_size.y * 0.10)
        };

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
    }

    fn execute_selected(&mut self, ctx: &egui::Context) {
        let command_count = self.command_count();
        if let Some(action) = fixed_action_for_index(self.palette.selected_index, command_count) {
            self.execute_fixed_action(ctx, action);
            return;
        }

        if let Some(orig_idx) = self
            .palette
            .filtered_commands
            .get(self.palette.selected_index)
            .map(|row| row.command_index)
        {
            self.activate_command(ctx, orig_idx, "execute_selected");
        }
    }

    fn activate_command(&mut self, ctx: &egui::Context, command_index: usize, reason: &str) {
        if self.command_behavior == CommandBehavior::Guide
            && self.palette.all_commands[command_index]
                .guide_hint
                .is_some()
        {
            self.start_guide(ctx, command_index, reason);
            return;
        }

        self.hide(ctx, reason);
        self.run_command_action(command_index);
    }

    fn run_command_action(&self, command_index: usize) {
        (self.palette.all_commands[command_index].action)();
        let _ = self.event_tx.send(UiEvent::ActionExecuted);
    }

    fn start_guide(&mut self, ctx: &egui::Context, command_index: usize, reason: &str) {
        let command = &self.palette.all_commands[command_index];
        let guide_hint = command
            .guide_hint
            .expect("guide commands should have guide metadata");
        let label = command.label.clone();
        let shortcut_text = command.shortcut_text.clone();
        let activation_hint = self.activation_hint.clone();
        let work_area = self.work_area;

        self.hide(ctx, reason);
        if let Some(target_window) = guide_hint.target_window {
            focus_window_token(target_window);
        }

        self.guide = Some(ActiveGuide {
            command_index,
            label,
            shortcut_text,
            activation_hint,
            work_area,
            expires_at: Instant::now() + GUIDE_DURATION,
        });
        let _ = self.event_tx.send(UiEvent::GuideStarted {
            shortcut: guide_hint.shortcut,
        });
        ctx.request_repaint();
    }

    fn run_guided_action(&mut self, ctx: &egui::Context) {
        let Some(guide) = self.guide.take() else {
            return;
        };

        close_guide_viewport(ctx);
        ctx.request_repaint();
        let _ = self.event_tx.send(UiEvent::GuideEnded);
        self.run_command_action(guide.command_index);
    }

    fn end_guide(&mut self, ctx: &egui::Context, send_event: bool) {
        if self.guide.take().is_some() {
            close_guide_viewport(ctx);
            ctx.request_repaint();
            if send_event {
                let _ = self.event_tx.send(UiEvent::GuideEnded);
            }
        }
    }

    fn refresh_guide(&mut self, ctx: &egui::Context) {
        let Some(guide) = self.guide.clone() else {
            return;
        };

        if Instant::now() >= guide.expires_at {
            self.end_guide(ctx, true);
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(100));
        show_guide_viewport(ctx, &guide);
    }

    fn execute_fixed_action(&mut self, ctx: &egui::Context, action: FixedPaletteAction) {
        self.end_guide(ctx, true);
        self.hide(ctx, "execute_fixed_action");
        match action {
            FixedPaletteAction::RefreshExtensions => {
                let _ = self.event_tx.send(UiEvent::ReloadExtensionsRequested);
            }
            FixedPaletteAction::OpenSettings => {
                self.handle_platform_action(ctx, PlatformUiAction::OpenSettings);
            }
        }
        let _ = self.event_tx.send(UiEvent::ActionExecuted);
    }

    fn command_count(&self) -> usize {
        self.palette.filtered_commands.len()
    }

    fn visible_command_row_count(&self) -> usize {
        self.command_count().min(MAX_VISIBLE_COMMAND_ROWS)
    }

    fn current_list_height(&self) -> f32 {
        self.command_list_height()
            + SETTINGS_DIVIDER_HEIGHT
            + (FIXED_ACTION_ROW_HEIGHT * FIXED_PALETTE_ACTIONS.len() as f32)
    }

    fn command_list_height(&self) -> f32 {
        let visible_command_count = self.visible_command_row_count();
        if visible_command_count == 0 {
            ROW_HEIGHT
        } else {
            visible_command_count as f32 * ESTIMATED_VISIBLE_ROW_HEIGHT
        }
    }

    fn handle_list_navigation_keys(&mut self, ctx: &egui::Context, visible_count: usize) {
        if visible_count == 0 {
            return;
        }

        let move_down =
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
        if move_down {
            self.palette.selected_index =
                wrapped_selection_index(self.palette.selected_index, visible_count, 1);
            self.keyboard_nav = true;
        }

        let move_up = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
        if move_up {
            self.palette.selected_index =
                wrapped_selection_index(self.palette.selected_index, visible_count, -1);
            self.keyboard_nav = true;
        }
    }
}

fn apply_platform_action(
    ctx: &egui::Context,
    settings: &Arc<Mutex<SettingsState>>,
    event_tx: &Sender<UiEvent>,
    action: PlatformUiAction,
) {
    match action {
        PlatformUiAction::OpenPalette => {
            let _ = event_tx.send(UiEvent::OpenPaletteRequested);
        }
        PlatformUiAction::OpenSettings => {
            if let Ok(mut settings) = settings.lock() {
                settings.open();
            }
            ctx.request_repaint();
        }
        PlatformUiAction::ReloadExtensions => {
            let _ = event_tx.send(UiEvent::ReloadExtensionsRequested);
        }
        PlatformUiAction::Quit => {
            let _ = event_tx.send(UiEvent::QuitRequested);
        }
    }
}

fn should_hide_for_app_switch(
    had_focus_since_open: bool,
    palette_window_token: Option<PlatformWindowToken>,
    foreground_window_token: Option<PlatformWindowToken>,
) -> bool {
    if !had_focus_since_open {
        return false;
    }

    matches!(
        (palette_window_token, foreground_window_token),
        (Some(palette_window_token), Some(foreground_window_token))
            if foreground_window_token != palette_window_token
    )
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(sig) = self.receiver.try_recv() {
            self.handle_signal(ctx, sig);
        }

        while let Some(action) = self.platform_ui.try_recv_action() {
            self.handle_platform_action(ctx, action);
        }

        if self.settings_open() {
            show_settings_viewport(ctx, Arc::clone(&self.settings), self.event_tx.clone());
        }

        self.refresh_guide(ctx);

        if !self.palette.is_open {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(16));

        self.sync_viewport(ctx);

        let is_focused = ctx.input(|i| i.focused);
        if is_focused {
            self.had_focus_since_open = true;
        }

        let palette_window_token = self.platform_ui.palette_window_token();
        let foreground_window_token = foreground_window_token();
        if should_hide_for_app_switch(
            self.had_focus_since_open,
            palette_window_token,
            foreground_window_token,
        ) {
            log::debug!(
                "Palette lost foreground window: egui_focused={}, palette_window_token={:?}, foreground_window_token={:?}",
                is_focused,
                palette_window_token,
                foreground_window_token
            );
            self.hide(ctx, "focus_loss");
            return;
        }

        if !is_focused && self.had_focus_since_open && foreground_window_token.is_none() {
            log::debug!(
                "Palette lost egui focus, but the platform foreground token is unavailable; keeping it open"
            );
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.hide(ctx, "escape");
            return;
        }

        let command_count = self.command_count();
        let selectable_count = command_count + FIXED_PALETTE_ACTIONS.len();
        self.handle_list_navigation_keys(ctx, selectable_count);

        if selectable_count > 0 {
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
                    .fill(PALETTE_CARD_BG)
                    .stroke(egui::Stroke::new(PALETTE_BORDER_WIDTH, PALETTE_BORDER))
                    .corner_radius(egui::CornerRadius::same(PALETTE_FRAME_RADIUS))
                    .inner_margin(egui::Margin::same(PALETTE_FRAME_MARGIN))
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .fill(PALETTE_SEARCH_BG)
                            .stroke(egui::Stroke::new(
                                PALETTE_BORDER_WIDTH,
                                PALETTE_SEARCH_BORDER,
                            ))
                            .corner_radius(egui::CornerRadius::same(PALETTE_SEARCH_RADIUS))
                            .inner_margin(egui::Margin {
                                left: PALETTE_SEARCH_MARGIN_X,
                                right: PALETTE_SEARCH_MARGIN_X,
                                top: PALETTE_SEARCH_MARGIN_Y,
                                bottom: PALETTE_SEARCH_MARGIN_Y,
                            })
                            .show(ui, |ui| {
                                ui.scope(|ui| {
                                    ui.visuals_mut().override_text_color =
                                        Some(PALETTE_TEXTBOX_TEXT);
                                    ui.visuals_mut().selection.bg_fill = PALETTE_ROW_SELECTED;
                                    ui.visuals_mut().selection.stroke = egui::Stroke::new(
                                        PALETTE_BORDER_WIDTH,
                                        PALETTE_ROW_SELECTED_BORDER,
                                    );
                                    ui.visuals_mut().text_cursor.stroke = egui::Stroke::new(
                                        PALETTE_CURSOR_WIDTH,
                                        PALETTE_TEXTBOX_CURSOR,
                                    );

                                    ui.horizontal(|ui| {
                                        ui.add_space(PALETTE_SEARCH_PROMPT_LEFT_SPACE);
                                        ui.label(
                                            egui::RichText::new(">")
                                                .size(PALETTE_SEARCH_PROMPT_SIZE)
                                                .color(PALETTE_TEXTBOX_HINT),
                                        );
                                        ui.add_space(PALETTE_SEARCH_PROMPT_RIGHT_SPACE);

                                        let resp = ui.add_sized(
                                            [ui.available_width(), PALETTE_SEARCH_HEIGHT],
                                            egui::TextEdit::singleline(
                                                &mut self.palette.filter_text,
                                            )
                                            .hint_text(
                                                egui::RichText::new("Type a command")
                                                    .color(PALETTE_TEXTBOX_HINT)
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

                        ui.add_space(PALETTE_RESULTS_TOP_SPACE);

                        let command_list_height = self.command_list_height();
                        let mut clicked_action: Option<usize> = None;

                        egui::ScrollArea::vertical()
                            .max_height(command_list_height)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                if self.palette.filtered_commands.is_empty() {
                                    egui::Frame::new()
                                        .fill(PALETTE_WINDOW_BG)
                                        .corner_radius(egui::CornerRadius::same(
                                            PALETTE_EMPTY_ROW_RADIUS,
                                        ))
                                        .inner_margin(egui::Margin::same(PALETTE_EMPTY_ROW_MARGIN))
                                        .show(ui, |ui| {
                                            ui.set_min_height(ROW_HEIGHT);
                                            ui.label(
                                                egui::RichText::new("No matching commands")
                                                    .size(PALETTE_EMPTY_TEXT_SIZE)
                                                    .color(PALETTE_TEXT_MUTED),
                                            );
                                        });
                                }

                                let rows: Vec<(usize, FilteredCommand)> = self
                                    .palette
                                    .filtered_commands
                                    .iter()
                                    .cloned()
                                    .enumerate()
                                    .collect();

                                for (idx, row) in rows {
                                    if let Some(clicked_idx) = self.palette.draw_command_row(
                                        ui,
                                        idx,
                                        &row,
                                        &mut self.keyboard_nav,
                                    ) {
                                        clicked_action = Some(clicked_idx);
                                    }
                                }
                            });

                        CommandPaletteApp::draw_settings_divider(ui);
                        for (fixed_idx, fixed_action) in FIXED_PALETTE_ACTIONS.iter().enumerate() {
                            let row_idx = command_count + fixed_idx;
                            if self.palette.draw_fixed_action_row(
                                ui,
                                row_idx,
                                *fixed_action,
                                &mut self.keyboard_nav,
                            ) {
                                self.execute_fixed_action(ui.ctx(), *fixed_action);
                                return;
                            }
                        }

                        if let Some(orig_idx) = clicked_action {
                            self.activate_command(ui.ctx(), orig_idx, "mouse_selection");
                        }
                    });
            });
    }

    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
}

pub fn run_with_shared_state(
    receiver: Receiver<UiSignal>,
    event_tx: Sender<UiEvent>,
    shared_context: SharedUiContext,
    visibility: SharedUiVisibility,
    settings_bootstrap: SettingsBootstrap,
) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([PALETTE_WIDTH, 700.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(true)
            .with_visible(false)
            .with_active(true),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Omni Palette",
        options,
        Box::new(move |cc| {
            Ok(Box::new(App::new(
                cc,
                receiver,
                event_tx,
                Arc::clone(&shared_context),
                Arc::clone(&visibility),
                settings_bootstrap,
            )))
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        apply_platform_action, should_hide_for_app_switch, PlatformUiAction, PlatformWindowToken,
        UiEvent,
    };
    use crate::config::runtime::RuntimeConfig;
    use crate::core::extensions::install::InstalledState;
    use crate::domain::action::Os;
    use crate::ui::settings::{SettingsBootstrap, SettingsState};
    use eframe::egui;
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};

    fn test_settings_state() -> Arc<Mutex<SettingsState>> {
        Arc::new(Mutex::new(SettingsState::new(SettingsBootstrap {
            config: RuntimeConfig::default(),
            config_path: None,
            config_error: None,
            current_os: Os::Windows,
            install_root: None,
            bundled_extensions: Vec::new(),
            installed_state: InstalledState::default(),
            installed_state_error: None,
        })))
    }

    #[test]
    fn app_switch_hide_stays_open_when_palette_is_still_foreground() {
        let token = PlatformWindowToken::new(100);
        assert!(!should_hide_for_app_switch(true, Some(token), Some(token)));
    }

    #[test]
    fn app_switch_hide_stays_open_before_palette_has_been_focused() {
        assert!(!should_hide_for_app_switch(
            false,
            Some(PlatformWindowToken::new(100)),
            Some(PlatformWindowToken::new(200)),
        ));
    }

    #[test]
    fn app_switch_hide_triggers_when_a_different_window_takes_foreground() {
        assert!(should_hide_for_app_switch(
            true,
            Some(PlatformWindowToken::new(100)),
            Some(PlatformWindowToken::new(200)),
        ));
    }

    #[test]
    fn app_switch_hide_ignores_missing_or_invalid_foreground_handles() {
        assert!(!should_hide_for_app_switch(
            true,
            Some(PlatformWindowToken::new(100)),
            None,
        ));
        assert!(!should_hide_for_app_switch(
            true,
            None,
            Some(PlatformWindowToken::new(200)),
        ));
    }

    #[test]
    fn apply_platform_action_opens_settings_locally() {
        let ctx = egui::Context::default();
        let settings = test_settings_state();
        let (event_tx, event_rx) = mpsc::channel();

        apply_platform_action(&ctx, &settings, &event_tx, PlatformUiAction::OpenSettings);

        assert!(settings
            .lock()
            .map(|settings| settings.open)
            .unwrap_or(false));
        assert!(event_rx.try_recv().is_err());
    }

    #[test]
    fn apply_platform_action_dispatches_open_palette_event() {
        let ctx = egui::Context::default();
        let settings = test_settings_state();
        let (event_tx, event_rx) = mpsc::channel();

        apply_platform_action(&ctx, &settings, &event_tx, PlatformUiAction::OpenPalette);

        assert!(matches!(
            event_rx.try_recv(),
            Ok(UiEvent::OpenPaletteRequested)
        ));
    }

    #[test]
    fn apply_platform_action_dispatches_reload_extensions_event() {
        let ctx = egui::Context::default();
        let settings = test_settings_state();
        let (event_tx, event_rx) = mpsc::channel();

        apply_platform_action(
            &ctx,
            &settings,
            &event_tx,
            PlatformUiAction::ReloadExtensions,
        );

        assert!(matches!(
            event_rx.try_recv(),
            Ok(UiEvent::ReloadExtensionsRequested)
        ));
    }

    #[test]
    fn apply_platform_action_dispatches_quit_event() {
        let ctx = egui::Context::default();
        let settings = test_settings_state();
        let (event_tx, event_rx) = mpsc::channel();

        apply_platform_action(&ctx, &settings, &event_tx, PlatformUiAction::Quit);

        assert!(matches!(event_rx.try_recv(), Ok(UiEvent::QuitRequested)));
    }
}
