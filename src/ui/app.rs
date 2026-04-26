use crate::config::runtime::RuntimeConfig;
use crate::core::command_filter::{
    filter_commands, initial_filtered_commands, FilterableCommand, FilteredCommand,
};
use crate::core::extensions::catalog::{CatalogEntry, ExtensionCatalog};
use crate::core::extensions::install::{BundledStaticExtension, InstalledState};
use crate::core::search::MatchRange;
use crate::domain::action::{CommandPriority, FocusState};
#[cfg(target_os = "windows")]
use crate::platform::windows::context::context::{
    foreground_window_handle_value, get_hwnd_from_raw,
};
use crate::ui::settings::{show_settings_viewport, SettingsBootstrap, SettingsState};
use eframe::egui;
use eframe::egui::text::LayoutJob;
#[cfg(target_os = "windows")]
use raw_window_handle::HasWindowHandle;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

#[cfg(target_os = "windows")]
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

const PALETTE_WIDTH: f32 = 780.0;
const MAX_FILTERED_COMMANDS: usize = 18;
const MAX_VISIBLE_COMMAND_ROWS: usize = 10;
const ROW_HEIGHT: f32 = 38.0;
const SETTINGS_DIVIDER_HEIGHT: f32 = 13.0;
const FIXED_ACTION_ROW_HEIGHT: f32 = 30.0;
const ESTIMATED_VISIBLE_ROW_HEIGHT: f32 = 31.0;
const FIXED_PALETTE_ACTIONS: [FixedPaletteAction; 2] = [
    FixedPaletteAction::RefreshExtensions,
    FixedPaletteAction::OpenSettings,
];

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
const TEXT_MATCH: egui::Color32 = egui::Color32::from_rgb(255, 213, 122);
const TEXT_MATCH_SELECTED: egui::Color32 = egui::Color32::from_rgb(255, 238, 180);
const HEART_ICON: &str = "♥";
const INDICATOR_SIZE: f32 = 12.0;
const HEART_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 106, 148);
const HEART_COLOR_SELECTED: egui::Color32 = egui::Color32::from_rgb(255, 190, 214);

pub struct Command {
    pub label: String,
    pub shortcut_text: String,
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

    fn width(self) -> f32 {
        self.right - self.left
    }

    fn height(self) -> f32 {
        self.bottom - self.top
    }

    fn to_points(self, native_pixels_per_point: f32) -> Self {
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
pub struct CommandPaletteApp {
    pub filter_text: String,
    pub all_commands: Vec<Command>,
    pub filtered_commands: Vec<FilteredCommand>,
    pub selected_index: usize,
    pub is_open: bool,
}

impl CommandPaletteApp {
    fn new(all_commands: Vec<Command>) -> Self {
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

    fn recompute_filter(&mut self) {
        self.filtered_commands =
            cap_filtered_commands(filter_commands(&self.all_commands, &self.filter_text));
    }
}

fn cap_filtered_commands(mut commands: Vec<FilteredCommand>) -> Vec<FilteredCommand> {
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
        TEXT_SELECTED
    } else {
        TEXT_PRIMARY
    };
    let highlight_color = if is_selected {
        TEXT_MATCH_SELECTED
    } else {
        TEXT_MATCH
    };
    let normal_format = egui::TextFormat {
        font_id: egui::FontId::proportional(15.5),
        color: normal_color,
        ..Default::default()
    };
    let highlight_format = egui::TextFormat {
        font_id: egui::FontId::proportional(15.5),
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
enum FixedPaletteAction {
    RefreshExtensions,
    OpenSettings,
}

impl FixedPaletteAction {
    fn label(self) -> &'static str {
        match self {
            Self::RefreshExtensions => "Refresh extensions",
            Self::OpenSettings => "Open settings for Omni Palette",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::RefreshExtensions => "fixed_refresh_extensions",
            Self::OpenSettings => "fixed_open_settings",
        }
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
    },
    Hide,
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
        extension: BundledStaticExtension,
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
    visibility: SharedUiVisibility,
    #[cfg(target_os = "windows")]
    palette_window_hwnd: Option<isize>,
    #[cfg(target_os = "windows")]
    _tray: Option<AppTray>,
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

        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.spacing.item_spacing = egui::vec2(0.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(0);
        cc.egui_ctx.set_global_style(style);
        let _ = shared_context.set(cc.egui_ctx.clone());
        visibility.store(false, Ordering::Relaxed);
        let settings = Arc::new(Mutex::new(SettingsState::new(settings_bootstrap)));
        #[cfg(target_os = "windows")]
        let palette_window_hwnd = palette_window_handle_value(cc);
        #[cfg(target_os = "windows")]
        let tray = AppTray::new(&cc.egui_ctx, event_tx.clone(), Arc::clone(&settings))
            .map_err(|err| {
                log::warn!("Could not create tray icon: {err}");
                err
            })
            .ok();

        Self {
            palette: CommandPaletteApp::new(vec![]),
            settings,
            receiver,
            event_tx,
            had_focus_since_open: false,
            needs_text_focus: false,
            keyboard_nav: false,
            work_area: None,
            visibility,
            #[cfg(target_os = "windows")]
            palette_window_hwnd,
            #[cfg(target_os = "windows")]
            _tray: tray,
        }
    }

    fn show(
        &mut self,
        ctx: &egui::Context,
        commands: Vec<Command>,
        work_area: Option<PaletteWorkArea>,
    ) {
        self.palette.all_commands = commands;
        self.palette.filter_text.clear();
        self.palette.selected_index = 0;
        self.palette.recompute_filter();
        self.palette.is_open = true;
        self.work_area = work_area;
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

    fn open_settings(&self, ctx: &egui::Context) {
        if let Ok(mut settings) = self.settings.lock() {
            settings.open();
        }
        ctx.request_repaint();
    }

    fn settings_open(&self) -> bool {
        self.settings
            .lock()
            .map(|settings| settings.open)
            .unwrap_or(false)
    }

    fn handle_signal(&mut self, ctx: &egui::Context, signal: UiSignal) {
        match signal {
            UiSignal::Show {
                commands,
                work_area,
            } => self.show(ctx, commands, work_area),
            UiSignal::Hide => self.hide(ctx, "signal"),
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
            self.hide(ctx, "execute_selected");
            (self.palette.all_commands[orig_idx].action)();
            let _ = self.event_tx.send(UiEvent::ActionExecuted);
        }
    }

    fn execute_fixed_action(&mut self, ctx: &egui::Context, action: FixedPaletteAction) {
        self.hide(ctx, "execute_fixed_action");
        match action {
            FixedPaletteAction::RefreshExtensions => {
                let _ = self.event_tx.send(UiEvent::ReloadExtensionsRequested);
            }
            FixedPaletteAction::OpenSettings => self.open_settings(ctx),
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

    fn draw_command_row(
        &mut self,
        ui: &mut egui::Ui,
        idx: usize,
        row: &FilteredCommand,
    ) -> Option<usize> {
        let is_selected = idx == self.palette.selected_index;
        let orig_idx = row.command_index;
        let label = &self.palette.all_commands[orig_idx].label;
        let shortcut_text = &self.palette.all_commands[orig_idx].shortcut_text;
        let is_favorite = self.palette.all_commands[orig_idx].favorite;
        let desired_size = egui::vec2(ui.available_width(), ROW_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

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
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(HEART_ICON).size(INDICATOR_SIZE).color(
                                if is_selected {
                                    HEART_COLOR_SELECTED
                                } else {
                                    HEART_COLOR
                                },
                            ));
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !shortcut_text.is_empty() {
                                ui.label(egui::RichText::new(shortcut_text).size(12.5).color(
                                    if is_selected {
                                        egui::Color32::from_rgb(180, 200, 230)
                                    } else {
                                        TEXT_MUTED
                                    },
                                ));
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

        if is_selected && self.keyboard_nav {
            click_response.scroll_to_me(Some(egui::Align::Center));
            self.keyboard_nav = false;
        }

        if click_response.clicked() {
            self.palette.selected_index = idx;
            return Some(orig_idx);
        }

        None
    }

    fn draw_settings_divider(ui: &mut egui::Ui) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(width, SETTINGS_DIVIDER_HEIGHT),
            egui::Sense::hover(),
        );
        let y = rect.center().y;
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + 8.0, y),
                egui::pos2(rect.right() - 8.0, y),
            ],
            egui::Stroke::new(1.0, BORDER),
        );
    }

    fn draw_fixed_action_row(
        &mut self,
        ui: &mut egui::Ui,
        idx: usize,
        action: FixedPaletteAction,
    ) -> bool {
        let is_selected = idx == self.palette.selected_index;
        let desired_size = egui::vec2(ui.available_width(), FIXED_ACTION_ROW_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

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

        let inner = rect.shrink2(egui::vec2(12.0, 4.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(action.label())
                        .size(15.5)
                        .color(if is_selected {
                            TEXT_SELECTED
                        } else {
                            TEXT_PRIMARY
                        }),
                );
            });
        });

        let click_response = ui
            .interact(rect, ui.id().with(action.id()), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        if is_selected && self.keyboard_nav {
            click_response.scroll_to_me(Some(egui::Align::Center));
            self.keyboard_nav = false;
        }

        if click_response.clicked() {
            self.palette.selected_index = idx;
            return true;
        }

        false
    }
}

fn wrapped_selection_index(current: usize, visible_count: usize, delta: isize) -> usize {
    if visible_count == 0 {
        return 0;
    }

    let visible_count = visible_count as isize;
    let current = current as isize;
    (current + delta).rem_euclid(visible_count) as usize
}

fn fixed_action_for_index(
    selected_index: usize,
    visible_command_count: usize,
) -> Option<FixedPaletteAction> {
    selected_index
        .checked_sub(visible_command_count)
        .and_then(|fixed_index| FIXED_PALETTE_ACTIONS.get(fixed_index).copied())
}

fn should_hide_for_app_switch(
    had_focus_since_open: bool,
    palette_window_hwnd: Option<isize>,
    foreground_window_hwnd: Option<isize>,
) -> bool {
    if !had_focus_since_open {
        return false;
    }

    matches!(
        (palette_window_hwnd, foreground_window_hwnd),
        (Some(palette_window_hwnd), Some(foreground_window_hwnd))
            if foreground_window_hwnd != palette_window_hwnd
    )
}

#[cfg(target_os = "windows")]
fn palette_window_handle_value(cc: &eframe::CreationContext<'_>) -> Option<isize> {
    let raw_window_handle = match cc.window_handle() {
        Ok(handle) => handle.as_raw(),
        Err(err) => {
            log::warn!("Could not obtain palette window handle: {err}");
            return None;
        }
    };
    let hwnd = match get_hwnd_from_raw(raw_window_handle) {
        Some(hwnd) => hwnd,
        None => {
            log::warn!("Palette window did not expose a Win32 window handle");
            return None;
        }
    };
    let hwnd_value = hwnd.0 as isize;
    log::debug!("Captured palette window handle: {:?}", hwnd);
    Some(hwnd_value)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(sig) = self.receiver.try_recv() {
            self.handle_signal(ctx, sig);
        }

        if self.settings_open() {
            show_settings_viewport(ctx, Arc::clone(&self.settings), self.event_tx.clone());
        }

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

        #[cfg(target_os = "windows")]
        {
            let foreground_window_hwnd = foreground_window_handle_value();
            if should_hide_for_app_switch(
                self.had_focus_since_open,
                self.palette_window_hwnd,
                foreground_window_hwnd,
            ) {
                log::debug!(
                    "Palette lost foreground window: egui_focused={}, palette_hwnd={:?}, foreground_hwnd={:?}",
                    is_focused,
                    self.palette_window_hwnd,
                    foreground_window_hwnd
                );
                self.hide(ctx, "app_switch");
                return;
            }

            if !is_focused && self.had_focus_since_open && foreground_window_hwnd.is_none() {
                log::debug!(
                    "Palette lost egui focus, but the foreground window handle is unavailable; keeping it open"
                );
            }
        }

        #[cfg(not(target_os = "windows"))]
        if !is_focused && self.had_focus_since_open {
            self.hide(ctx, "focus_loss");
            return;
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
                                top: 14,
                                bottom: 14,
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

                        let command_list_height = self.command_list_height();
                        let mut clicked_action: Option<usize> = None;

                        egui::ScrollArea::vertical()
                            .max_height(command_list_height)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                if self.palette.filtered_commands.is_empty() {
                                    egui::Frame::new()
                                        .fill(BG)
                                        .corner_radius(egui::CornerRadius::same(6))
                                        .inner_margin(egui::Margin::same(12))
                                        .show(ui, |ui| {
                                            ui.set_min_height(ROW_HEIGHT);
                                            ui.label(
                                                egui::RichText::new("No matching commands")
                                                    .size(14.5)
                                                    .color(TEXT_MUTED),
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
                                    if let Some(clicked_idx) = self.draw_command_row(ui, idx, &row)
                                    {
                                        clicked_action = Some(clicked_idx);
                                    }
                                }
                            });

                        Self::draw_settings_divider(ui);
                        for (fixed_idx, fixed_action) in FIXED_PALETTE_ACTIONS.iter().enumerate() {
                            let row_idx = command_count + fixed_idx;
                            if self.draw_fixed_action_row(ui, row_idx, *fixed_action) {
                                self.execute_fixed_action(ui.ctx(), *fixed_action);
                                return;
                            }
                        }

                        if let Some(orig_idx) = clicked_action {
                            self.hide(ui.ctx(), "mouse_selection");
                            (self.palette.all_commands[orig_idx].action)();
                            let _ = self.event_tx.send(UiEvent::ActionExecuted);
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

#[cfg(target_os = "windows")]
struct AppTray {
    _tray: TrayIcon,
}

#[cfg(target_os = "windows")]
impl AppTray {
    fn new(
        ctx: &egui::Context,
        event_tx: Sender<UiEvent>,
        settings_state: Arc<Mutex<SettingsState>>,
    ) -> Result<Self, String> {
        let menu = Menu::new();
        let open_palette = MenuItem::new("Open Palette", true, None);
        let settings = MenuItem::new("Settings...", true, None);
        let reload = MenuItem::new("Reload Extensions", true, None);
        let quit = MenuItem::new("Quit", true, None);
        menu.append(&open_palette).map_err(|err| err.to_string())?;
        menu.append(&settings).map_err(|err| err.to_string())?;
        menu.append(&reload).map_err(|err| err.to_string())?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|err| err.to_string())?;
        menu.append(&quit).map_err(|err| err.to_string())?;

        let ctx = ctx.clone();
        let open_palette_id = open_palette.id().clone();
        let settings_id = settings.id().clone();
        let reload_id = reload.id().clone();
        let quit_id = quit.id().clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if open_palette_id == event.id() {
                let _ = event_tx.send(UiEvent::OpenPaletteRequested);
            } else if settings_id == event.id() {
                if let Ok(mut settings) = settings_state.lock() {
                    settings.open();
                }
            } else if reload_id == event.id() {
                let _ = event_tx.send(UiEvent::ReloadExtensionsRequested);
            } else if quit_id == event.id() {
                let _ = event_tx.send(UiEvent::QuitRequested);
            }
            ctx.request_repaint();
        }));

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Omni Palette")
            .with_icon(tray_icon()?)
            .build()
            .map_err(|err| err.to_string())?;

        Ok(Self { _tray: tray })
    }
}

#[cfg(target_os = "windows")]
fn tray_icon() -> Result<Icon, String> {
    let size = 16_u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let border = x == 0 || y == 0 || x == size - 1 || y == size - 1;
            let diagonal = x == y || x + y == size - 1;
            let (r, g, b) = if border {
                (230, 230, 230)
            } else if diagonal {
                (66, 153, 225)
            } else {
                (28, 32, 40)
            };
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Icon::from_rgba(rgba, size, size).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        cap_filtered_commands, fixed_action_for_index, should_hide_for_app_switch,
        wrapped_selection_index, FilteredCommand, FixedPaletteAction, MAX_FILTERED_COMMANDS,
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

    #[test]
    fn app_switch_hide_stays_open_when_palette_is_still_foreground() {
        assert!(!should_hide_for_app_switch(true, Some(100), Some(100)));
    }

    #[test]
    fn app_switch_hide_stays_open_before_palette_has_been_focused() {
        assert!(!should_hide_for_app_switch(false, Some(100), Some(200)));
    }

    #[test]
    fn app_switch_hide_triggers_when_a_different_window_takes_foreground() {
        assert!(should_hide_for_app_switch(true, Some(100), Some(200)));
    }

    #[test]
    fn app_switch_hide_ignores_missing_or_invalid_foreground_handles() {
        assert!(!should_hide_for_app_switch(true, Some(100), None));
        assert!(!should_hide_for_app_switch(true, None, Some(200)));
    }
}
