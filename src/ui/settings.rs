use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::config::runtime::{CommandBehavior, GitHubExtensionSource, RuntimeConfig};
use crate::core::extensions::catalog::{CatalogEntry, ExtensionCatalog, ExtensionKind};
use crate::core::extensions::install::{
    BundledExtension, InstalledExtension, InstalledState, BUNDLED_SOURCE_ID, GITHUB_SOURCE_ID,
};
use crate::core::extensions::settings::{
    extension_settings_key, ExtensionSettingItem, ExtensionSettingsCategory,
    ExtensionSettingsSchema, ExtensionSettingsTarget, ExtensionSettingsValues,
    LoadedExtensionSettings, SavedExtensionSettings,
};
use crate::domain::action::Os;
use crate::domain::hotkey::{HotkeyModifiers, Key, KeyboardShortcut};
use crate::theme::{current_app_theme, SettingsTheme, ThemeMode};
use crate::ui::app::{InstalledExtensionsUpdate, UiEvent};
use crate::ui::components::toggle_switch;

const SETTINGS_VIEWPORT_ID: &str = "omni_palette_settings";
const SETTINGS_WIDTH: f32 = 1180.0;
const SETTINGS_HEIGHT: f32 = 840.0;
const SIDEBAR_WIDTH: f32 = 220.0;
const ROW_LABEL_WIDTH: f32 = 148.0;
const SETTING_ROW_HEIGHT: f32 = 30.0;
const HELP_ICON_DIAMETER: f32 = 15.0;
const HELP_ICON_GAP: f32 = 6.0;
const RADIO_OPTION_SPACING: f32 = 14.0;
const TEXT_INPUT_WIDTH: f32 = 480.0;
const CATALOG_ROW_HEIGHT: f32 = 76.0;
const CATALOG_MIN_VISIBLE_ROWS: f32 = 3.0;
const CATALOG_MIN_HEIGHT: f32 = CATALOG_ROW_HEIGHT * CATALOG_MIN_VISIBLE_ROWS;
const CATALOG_MAX_HEIGHT: f32 = 300.0;
const ACTION_BUTTON_SPACING: f32 = 12.0;
const EXTENSION_SOURCE_ROW_SPACING: f32 = 12.0;
const EXTENSION_STATUS_LABEL_WIDTH: f32 = 66.0;
const EXTENSION_ACTION_SLOT_WIDTH: f32 = 188.0;
const STATUS_TOAST_DURATION: Duration = Duration::from_secs(3);

pub fn settings_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of(SETTINGS_VIEWPORT_ID)
}

#[derive(Debug, Clone)]
pub struct SettingsBootstrap {
    pub config: RuntimeConfig,
    pub config_path: Option<PathBuf>,
    pub config_error: Option<String>,
    pub current_os: Os,
    pub install_root: Option<PathBuf>,
    pub bundled_extensions: Vec<BundledExtension>,
    pub extension_settings_available: HashSet<String>,
    pub installed_state: InstalledState,
    pub installed_state_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    Installed,
    Marketplace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingUninstall {
    extension_id: String,
    source_id: String,
}

#[derive(Debug, Clone)]
struct SettingsToast {
    message: String,
    created_at: Instant,
}

#[derive(Debug, Clone)]
struct ExtensionSettingsPanelState {
    target: ExtensionSettingsTarget,
    schema: ExtensionSettingsSchema,
    draft_values: ExtensionSettingsValues,
    saved_values: ExtensionSettingsValues,
    expanded_categories: HashSet<String>,
    saving: bool,
}

impl ExtensionSettingsPanelState {
    fn is_dirty(&self) -> bool {
        self.draft_values != self.saved_values
    }
}

#[derive(Debug)]
pub struct SettingsState {
    pub open: bool,
    focus_requested: bool,
    tab: SettingsTab,
    config_path: Option<PathBuf>,
    config_error: Option<String>,
    current_os: Os,
    install_root: Option<PathBuf>,
    bundled_extensions: Vec<BundledExtension>,
    extension_settings_available: HashSet<String>,
    draft: RuntimeConfig,
    saved: RuntimeConfig,
    installed_state: InstalledState,
    installed_state_error: Option<String>,
    catalog: Option<ExtensionCatalog>,
    catalog_error: Option<String>,
    catalog_busy: bool,
    catalog_filter_text: String,
    extension_busy: Option<String>,
    pending_uninstall: Option<PendingUninstall>,
    loading_extension_settings_key: Option<String>,
    extension_settings_panel: Option<ExtensionSettingsPanelState>,
    saving: bool,
    pending_catalog_source_enabled_save: Option<bool>,
    recording_hotkey: bool,
    status: Option<SettingsToast>,
}

impl SettingsState {
    pub fn new(bootstrap: SettingsBootstrap) -> Self {
        Self {
            open: false,
            focus_requested: false,
            tab: SettingsTab::General,
            config_path: bootstrap.config_path,
            config_error: bootstrap.config_error,
            current_os: bootstrap.current_os,
            install_root: bootstrap.install_root,
            bundled_extensions: bootstrap.bundled_extensions,
            extension_settings_available: bootstrap.extension_settings_available,
            draft: bootstrap.config.clone(),
            saved: bootstrap.config,
            installed_state: bootstrap.installed_state,
            installed_state_error: bootstrap.installed_state_error,
            catalog: None,
            catalog_error: None,
            catalog_busy: false,
            catalog_filter_text: String::new(),
            extension_busy: None,
            pending_uninstall: None,
            loading_extension_settings_key: None,
            extension_settings_panel: None,
            saving: false,
            pending_catalog_source_enabled_save: None,
            recording_hotkey: false,
            status: None,
        }
    }

    pub fn open(&mut self) {
        self.open = true;
        self.focus_requested = true;
    }

    fn take_focus_request(&mut self) -> bool {
        let requested = self.focus_requested;
        self.focus_requested = false;
        requested
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some(SettingsToast {
            message: message.into(),
            created_at: Instant::now(),
        });
    }

    pub fn config_saved(&mut self, config: RuntimeConfig, result: Result<String, String>) {
        self.saving = false;
        let pending_catalog_source_enabled_save = self.pending_catalog_source_enabled_save.take();
        match result {
            Ok(message) => {
                self.saved = config.clone();
                self.draft = config;
                self.config_error = None;
                self.set_status(message);
            }
            Err(err) => {
                if let Some(previous_enabled) = pending_catalog_source_enabled_save {
                    self.draft.github.enabled = previous_enabled;
                }
                self.set_status(err);
            }
        }
    }

    pub fn catalog_refreshed(&mut self, result: Result<ExtensionCatalog, String>) {
        self.catalog_busy = false;
        match result {
            Ok(catalog) => {
                let supported_count = catalog
                    .entries
                    .iter()
                    .filter(|entry| entry.platform == self.current_os)
                    .count();
                self.catalog = Some(catalog);
                self.catalog_error = None;
                self.set_status(format!(
                    "Catalog refreshed: {supported_count} {} extensions available",
                    os_label(self.current_os)
                ));
            }
            Err(err) => {
                self.catalog_error = Some(err.clone());
                self.set_status(err);
            }
        }
    }

    pub fn installed_extensions_updated(
        &mut self,
        result: Result<InstalledExtensionsUpdate, String>,
    ) {
        self.extension_busy = None;
        self.pending_uninstall = None;
        match result {
            Ok(update) => {
                self.installed_state = update.state;
                self.extension_settings_available = update.extension_settings_available;
                self.sync_bundled_extension_enabled();
                self.installed_state_error = None;
                self.set_status(update.message);
            }
            Err(err) => self.set_status(err),
        }
    }

    pub fn reload_finished(&mut self, result: Result<String, String>) {
        self.set_status(match result {
            Ok(message) => message,
            Err(err) => err,
        });
    }

    pub fn extension_settings_loaded(&mut self, result: Result<LoadedExtensionSettings, String>) {
        self.loading_extension_settings_key = None;
        match result {
            Ok(loaded) => {
                let expanded_categories = default_expanded_categories(&loaded.schema);
                self.extension_settings_panel = Some(ExtensionSettingsPanelState {
                    target: loaded.target,
                    schema: loaded.schema,
                    draft_values: loaded.values.clone(),
                    saved_values: loaded.values,
                    expanded_categories,
                    saving: false,
                });
            }
            Err(err) => self.set_status(err),
        }
    }

    pub fn extension_settings_saved(&mut self, result: Result<SavedExtensionSettings, String>) {
        match result {
            Ok(saved) => {
                if self
                    .extension_settings_panel
                    .as_ref()
                    .is_some_and(|panel| panel.target.key() == saved.target.key())
                {
                    self.extension_settings_panel = None;
                }
                self.set_status(saved.message);
            }
            Err(err) => {
                if let Some(panel) = &mut self.extension_settings_panel {
                    panel.saving = false;
                }
                self.set_status(err);
            }
        }
    }

    fn is_dirty(&self) -> bool {
        self.draft != self.saved
    }

    fn has_visible_unsaved_settings_changes(&self) -> bool {
        self.is_dirty() && !self.is_pending_catalog_source_enabled_only_change()
    }

    fn is_pending_catalog_source_enabled_only_change(&self) -> bool {
        let Some(previous_enabled) = self.pending_catalog_source_enabled_save else {
            return false;
        };

        let mut draft_without_pending_toggle = self.draft.clone();
        draft_without_pending_toggle.github.enabled = previous_enabled;
        draft_without_pending_toggle == self.saved
    }

    fn save_catalog_source_enabled_change(
        &mut self,
        previous_enabled: bool,
        event_tx: &Sender<UiEvent>,
    ) {
        if previous_enabled == self.draft.github.enabled {
            return;
        }

        if self.config_path.is_none() {
            self.draft.github.enabled = previous_enabled;
            self.set_status("APPDATA is not available, so settings cannot be saved.");
            return;
        }

        if self.saving {
            self.draft.github.enabled = previous_enabled;
            self.set_status("Settings are already being saved.");
            return;
        }

        self.saving = true;
        self.pending_catalog_source_enabled_save = Some(previous_enabled);
        if event_tx
            .send(UiEvent::SaveRuntimeConfigRequested(self.draft.clone()))
            .is_ok()
        {
            self.set_status("Saving catalog source");
        } else {
            self.saving = false;
            self.pending_catalog_source_enabled_save = None;
            self.draft.github.enabled = previous_enabled;
            self.set_status(
                "Could not save catalog source because the settings channel is unavailable.",
            );
        }
    }

    fn sync_bundled_extension_enabled(&mut self) {
        for extension in &mut self.bundled_extensions {
            extension.enabled = self
                .installed_state
                .enabled_for(&extension.id, BUNDLED_SOURCE_ID)
                .unwrap_or(true);
        }
    }

    fn has_extension_settings(&self, extension_id: &str, source_id: &str) -> bool {
        self.extension_settings_available
            .contains(&extension_settings_key(extension_id, source_id))
    }

    fn settings_target_for_bundled_extension(
        &self,
        extension: &BundledExtension,
    ) -> ExtensionSettingsTarget {
        ExtensionSettingsTarget {
            extension_id: extension.id.clone(),
            source_id: BUNDLED_SOURCE_ID.to_string(),
            display_name: extension.name.clone(),
            kind: extension.kind,
            installed_path: extension.installed_path.clone(),
        }
    }

    fn settings_target_for_installed_extension(
        &self,
        extension: &InstalledExtension,
        display_name: String,
    ) -> ExtensionSettingsTarget {
        let installed_path = match (&self.install_root, extension.installed_path.is_absolute()) {
            (_, true) => extension.installed_path.clone(),
            (Some(root), false) => root.join(&extension.installed_path),
            (None, false) => extension.installed_path.clone(),
        };

        ExtensionSettingsTarget {
            extension_id: extension.id.clone(),
            source_id: extension.source_id.clone(),
            display_name,
            kind: extension.kind,
            installed_path,
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui, event_tx: &Sender<UiEvent>) {
        if ui.ctx().input(|input| input.viewport().close_requested()) {
            self.open = false;
            return;
        }

        if self.recording_hotkey {
            if let Some(shortcut) = capture_shortcut(ui.ctx()) {
                self.draft.activation = shortcut;
                self.recording_hotkey = false;
                self.set_status(format!("Recorded {}", shortcut));
            }
        }

        ui.scope(|ui| {
            let theme = settings_theme(ui);
            apply_settings_visuals(ui);
            let size = ui.available_size();

            egui::Frame::new()
                .fill(theme.bg)
                .inner_margin(egui::Margin::same(0))
                .show(ui, |ui| {
                    ui.set_min_size(size);
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    ui.allocate_ui_with_layout(
                        size,
                        egui::Layout::left_to_right(egui::Align::TOP),
                        |ui| {
                            let height = ui.available_height();
                            ui.allocate_ui_with_layout(
                                egui::vec2(SIDEBAR_WIDTH, height),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| self.draw_sidebar(ui),
                            );

                            let content_width = ui.available_width();
                            ui.allocate_ui_with_layout(
                                egui::vec2(content_width, height),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| self.draw_content(ui, event_tx),
                            );
                        },
                    );
                });
        });
        self.draw_status_toast(ui.ctx());
        self.draw_extension_settings_panel(ui.ctx(), event_tx);
    }

    fn draw_sidebar(&mut self, ui: &mut egui::Ui) {
        let theme = settings_theme(ui);
        egui::Frame::new()
            .fill(theme.sidebar_bg)
            .inner_margin(egui::Margin {
                left: 20,
                right: 20,
                top: 24,
                bottom: 24,
            })
            .show(ui, |ui| {
                ui.set_width(SIDEBAR_WIDTH - 40.0);
                ui.set_min_height(ui.available_height());
                ui.label(
                    egui::RichText::new("Omni Palette")
                        .size(20.0)
                        .strong()
                        .color(theme.text_primary),
                );
                selectable_label(
                    ui,
                    settings_text(theme, "Preferences", SettingsTextTone::Muted).size(12.0),
                );

                ui.add_space(24.0);
                if nav_button(
                    ui,
                    self.tab == SettingsTab::General,
                    "General",
                    "Shortcut and config",
                )
                .clicked()
                {
                    self.tab = SettingsTab::General;
                }
                ui.add_space(8.0);
                if nav_button(
                    ui,
                    self.tab == SettingsTab::Installed,
                    "Manage Extensions",
                    "Enable and remove",
                )
                .clicked()
                {
                    self.tab = SettingsTab::Installed;
                }
                ui.add_space(8.0);
                if nav_button(
                    ui,
                    self.tab == SettingsTab::Marketplace,
                    "Marketplace",
                    "Browse and install",
                )
                .clicked()
                {
                    self.tab = SettingsTab::Marketplace;
                }
            });
    }

    fn draw_content(&mut self, ui: &mut egui::Ui, event_tx: &Sender<UiEvent>) {
        let theme = settings_theme(ui);
        egui::Frame::new()
            .fill(theme.bg)
            .inner_margin(egui::Margin {
                left: 28,
                right: 28,
                top: 26,
                bottom: 24,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(ui.available_height());

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        match self.tab {
                            SettingsTab::General => self.draw_general(ui, event_tx),
                            SettingsTab::Installed => self.draw_installed_page(ui, event_tx),
                            SettingsTab::Marketplace => self.draw_marketplace(ui, event_tx),
                        }
                    });
            });
    }

    fn draw_status_summary(&self, ui: &mut egui::Ui) {
        if self.has_visible_unsaved_settings_changes() {
            banner(
                ui,
                "You have unsaved settings changes. Save to make them your new defaults.",
                BannerTone::Warning,
            );
            ui.add_space(12.0);
        }
    }

    fn draw_status_toast(&mut self, ctx: &egui::Context) {
        let Some(status) = &self.status else {
            return;
        };

        if status.created_at.elapsed() >= STATUS_TOAST_DURATION {
            self.status = None;
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(100));
        let message = status.message.clone();

        egui::Area::new(egui::Id::new("settings_status_toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -24.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| toast(ui, &message));
    }

    fn draw_general(&mut self, ui: &mut egui::Ui, event_tx: &Sender<UiEvent>) {
        page_header(
            ui,
            "General",
            "Control how Omni Palette opens and where your personal preferences are stored.",
        );
        self.draw_status_summary(ui);

        section(
            ui,
            "Appearance",
            "Choose whether Omni Palette follows Windows or uses a fixed warm light or dark theme.",
            |ui| {
                setting_row_with_help(
                    ui,
                    "Theme",
                    "System follows the OS preference when egui reports it. If the OS theme is unavailable, Omni Palette falls back to dark mode.",
                    |ui| {
                        radio_option_row(ui, |ui| {
                            ui.radio_value(
                                &mut self.draft.appearance.theme,
                                ThemeMode::System,
                                "System",
                            );
                            ui.radio_value(
                                &mut self.draft.appearance.theme,
                                ThemeMode::Light,
                                "Light",
                            );
                            ui.radio_value(
                                &mut self.draft.appearance.theme,
                                ThemeMode::Dark,
                                "Dark",
                            );
                        });
                    },
                );
            },
        );

        section(
            ui,
            "Activation",
            "The global shortcut should feel memorable, quick, and hard to press by accident.",
            |ui| {
                setting_row(ui, "Shortcut", |ui| {
                    action_button_row(ui, |ui| {
                        shortcut_pill(ui, &self.draft.activation.to_string());
                        if secondary_button(ui, "Record").clicked() {
                            self.recording_hotkey = true;
                            self.set_status("Press the new activation shortcut");
                        }
                        if secondary_button(ui, "Reset").clicked() {
                            self.draft.activation = RuntimeConfig::default_activation_shortcut();
                            self.recording_hotkey = false;
                            self.set_status("Activation hotkey reset to the code default");
                        }
                    });
                });

                if self.recording_hotkey {
                    ui.add_space(10.0);
                    banner(
                        ui,
                        "Listening for a shortcut. Press the new key combination now.",
                        BannerTone::Info,
                    );
                }
            },
        );

        section(
            ui,
            "Command Behavior",
            "Choose what happens when you select a command.",
            |ui| {
                setting_row_with_help(
                    ui,
                    "Mode",
                    "Execute runs commands immediately. Guide shows the native shortcut first; press the activation shortcut again to run it for you.",
                    |ui| {
                        radio_option_row(ui, |ui| {
                            ui.radio_value(
                                &mut self.draft.command_behavior,
                                CommandBehavior::Execute,
                                "Execute",
                            );
                            ui.radio_value(
                                &mut self.draft.command_behavior,
                                CommandBehavior::Guide,
                                "Guide",
                            );
                        });
                    },
                );
            },
        );

        section(
            ui,
            "Storage",
            "Settings are saved as TOML so they are easy to inspect, back up, and repair.",
            |ui| {
                if let Some(path) = &self.config_path {
                    setting_row(ui, "User config", |ui| {
                        path_chip(ui, &path.display().to_string())
                    });
                } else {
                    banner(
                        ui,
                        "APPDATA is not available, so settings cannot be saved.",
                        BannerTone::Warning,
                    );
                }

                if let Some(error) = &self.config_error {
                    ui.add_space(10.0);
                    banner(
                        ui,
                        &format!("Config load warning: {error}"),
                        BannerTone::Error,
                    );
                }
            },
        );

        save_bar(ui, self.is_dirty(), self.saving, |ui| {
            let can_save = self.config_path.is_some() && self.is_dirty() && !self.saving;
            if ui
                .add_enabled(can_save, primary_button(ui, "Save Settings"))
                .clicked()
            {
                self.saving = true;
                self.set_status("Saving settings");
                let _ = event_tx.send(UiEvent::SaveRuntimeConfigRequested(self.draft.clone()));
            }

            if ui
                .add_enabled(
                    self.is_dirty() && !self.saving,
                    secondary_button_widget(ui, "Discard Changes"),
                )
                .clicked()
            {
                self.draft = self.saved.clone();
                self.recording_hotkey = false;
                self.set_status("Discarded unsaved changes");
            }
        });
    }

    fn draw_installed_page(&mut self, ui: &mut egui::Ui, event_tx: &Sender<UiEvent>) {
        page_header(
            ui,
            "Installed Extensions",
            "Manage extensions that are available on this device.",
        );
        self.draw_status_summary(ui);

        section(
            ui,
            "Bundled Defaults",
            "Built into Omni Palette. They can be disabled, but not uninstalled.",
            |ui| self.draw_bundled_extensions(ui, event_tx),
        );

        section(
            ui,
            "Downloaded Extensions",
            "Installed from your configured catalog.",
            |ui| self.draw_downloaded_extensions(ui, event_tx),
        );
    }

    fn draw_marketplace(&mut self, ui: &mut egui::Ui, event_tx: &Sender<UiEvent>) {
        page_header(
            ui,
            "Extension Marketplace",
            "Install extensions from a GitHub catalog you trust.",
        );
        self.draw_status_summary(ui);

        section(
            ui,
            "Catalog Source",
            "Choose the GitHub catalog Omni Palette should refresh from.",
            |ui| self.draw_extension_source(ui, event_tx),
        );

        section(
            ui,
            "Available Extensions",
            "Search the refreshed catalog for extensions that support this Windows build.",
            |ui| self.draw_catalog(ui, event_tx),
        );
    }

    fn draw_extension_source(&mut self, ui: &mut egui::Ui, event_tx: &Sender<UiEvent>) {
        let previous_enabled = self.draft.github.enabled;
        let toggle_enabled = self.config_path.is_some() && !self.saving;
        let (toggle_response, _, _, _) =
            extension_source_status_row(ui, &mut self.draft.github.enabled, toggle_enabled);
        if toggle_response.changed() {
            self.save_catalog_source_enabled_change(previous_enabled, event_tx);
        }
        ui.add_space(12.0);
        edit_row(ui, "Owner", &mut self.draft.github.owner);
        edit_row(ui, "Repo", &mut self.draft.github.repo);
        edit_row(ui, "Branch", &mut self.draft.github.branch);
        edit_row(ui, "Catalog path", &mut self.draft.github.catalog_path);
        ui.add_space(14.0);
        action_button_row(ui, |ui| {
            let can_save = self.config_path.is_some() && self.is_dirty() && !self.saving;
            if ui
                .add_enabled(can_save, primary_button(ui, "Save Source"))
                .clicked()
            {
                self.saving = true;
                self.set_status("Saving settings");
                let _ = event_tx.send(UiEvent::SaveRuntimeConfigRequested(self.draft.clone()));
            }

            let can_refresh =
                self.draft.github.enabled && self.install_root.is_some() && !self.catalog_busy;
            if ui
                .add_enabled(can_refresh, secondary_button_widget(ui, "Refresh Catalog"))
                .clicked()
            {
                self.catalog_error = None;
                match validate_catalog_source(&self.draft.github) {
                    Ok(()) => {
                        self.catalog_busy = true;
                        self.set_status("Refreshing extension catalog");
                        let _ = event_tx
                            .send(UiEvent::RefreshCatalogRequested(self.draft.github.clone()));
                    }
                    Err(err) => {
                        self.catalog_error = Some(err.clone());
                        self.set_status(err);
                    }
                }
            }

            if secondary_button(ui, "Reload Extensions").clicked() {
                self.set_status("Reloading extensions");
                let _ = event_tx.send(UiEvent::ReloadExtensionsRequested);
            }
        });

        if let Some(error) = &self.catalog_error {
            ui.add_space(10.0);
            banner(ui, &format!("Catalog error: {error}"), BannerTone::Error);
        }
    }

    fn draw_bundled_extensions(&mut self, ui: &mut egui::Ui, event_tx: &Sender<UiEvent>) {
        let theme = settings_theme(ui);
        if let Some(error) = &self.installed_state_error {
            banner(
                ui,
                &format!("Installed extension warning: {error}"),
                BannerTone::Error,
            );
            ui.add_space(10.0);
        }

        if self.bundled_extensions.is_empty() {
            empty_state(ui, "No bundled defaults are available.");
            return;
        }

        let bundled_extensions = self.bundled_extensions.clone();
        for extension in bundled_extensions {
            list_row(ui, |ui| {
                ui.vertical(|ui| {
                    extension_title_with_version(ui, &extension.name, &extension.version);
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        badge(ui, "Bundled", theme.text_muted);
                        badge(ui, extension_kind_badge(extension.kind), theme.text_muted);
                    });
                });

                let busy_key = extension_busy_key(&extension.id, BUNDLED_SOURCE_ID);
                let busy = self.extension_busy.as_deref() == Some(busy_key.as_str());
                let settings_key = extension_settings_key(&extension.id, BUNDLED_SOURCE_ID);
                let loading_settings =
                    self.loading_extension_settings_key.as_deref() == Some(settings_key.as_str());
                let has_settings = self.has_extension_settings(&extension.id, BUNDLED_SOURCE_ID);
                let mut enabled = extension.enabled;
                extension_enabled_actions(ui, extension.enabled, &mut enabled, !busy, |ui| {
                    if has_settings
                        && ui
                            .add_enabled(
                                !loading_settings,
                                secondary_button_widget(
                                    ui,
                                    if loading_settings {
                                        "Loading..."
                                    } else {
                                        "Settings"
                                    },
                                ),
                            )
                            .clicked()
                    {
                        self.loading_extension_settings_key = Some(settings_key);
                        let _ = event_tx.send(UiEvent::OpenExtensionSettingsRequested {
                            target: self.settings_target_for_bundled_extension(&extension),
                        });
                    }
                });
                if enabled != extension.enabled {
                    self.extension_busy = Some(busy_key);
                    let _ = event_tx.send(UiEvent::SetBundledExtensionEnabledRequested {
                        extension: extension.clone(),
                        enabled,
                    });
                }
            });
        }
    }

    fn draw_downloaded_extensions(&mut self, ui: &mut egui::Ui, event_tx: &Sender<UiEvent>) {
        let theme = settings_theme(ui);
        let installed_extensions = self
            .installed_state
            .extensions
            .iter()
            .filter(|extension| extension.source_id != BUNDLED_SOURCE_ID)
            .cloned()
            .collect::<Vec<_>>();

        if installed_extensions.is_empty() {
            empty_state(ui, "No downloaded extensions installed yet.");
            return;
        }

        for extension in installed_extensions {
            let display_name = self.installed_extension_display_name(&extension);
            let busy_key = extension_busy_key(&extension.id, &extension.source_id);
            let busy = self.extension_busy.as_deref() == Some(busy_key.as_str());
            let pending_uninstall = self.is_uninstall_pending(&extension.id, &extension.source_id);
            let settings_key = extension_settings_key(&extension.id, &extension.source_id);
            let loading_settings =
                self.loading_extension_settings_key.as_deref() == Some(settings_key.as_str());
            let has_settings = self.has_extension_settings(&extension.id, &extension.source_id);

            list_row(ui, |ui| {
                ui.vertical(|ui| {
                    extension_title_with_version(ui, &display_name, &extension.version);
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        badge(ui, "Downloaded", theme.text_muted);
                    });
                });

                if pending_uninstall {
                    right_aligned_actions(ui, |ui| {
                        self.draw_uninstall_confirmation(
                            ui,
                            event_tx,
                            &extension,
                            &display_name,
                            busy,
                        );
                    });
                } else {
                    let mut enabled = extension.enabled;
                    extension_enabled_actions(ui, extension.enabled, &mut enabled, !busy, |ui| {
                        if ui
                            .add_enabled(!busy, danger_button(ui, "Uninstall"))
                            .clicked()
                        {
                            self.pending_uninstall = Some(PendingUninstall {
                                extension_id: extension.id.clone(),
                                source_id: extension.source_id.clone(),
                            });
                        }

                        if has_settings
                            && ui
                                .add_enabled(
                                    !loading_settings,
                                    secondary_button_widget(
                                        ui,
                                        if loading_settings {
                                            "Loading..."
                                        } else {
                                            "Settings"
                                        },
                                    ),
                                )
                                .clicked()
                        {
                            self.loading_extension_settings_key = Some(settings_key);
                            let _ = event_tx.send(UiEvent::OpenExtensionSettingsRequested {
                                target: self.settings_target_for_installed_extension(
                                    &extension,
                                    display_name.clone(),
                                ),
                            });
                        }
                    });
                    if enabled != extension.enabled {
                        self.extension_busy = Some(busy_key);
                        let _ = event_tx.send(UiEvent::SetExtensionEnabledRequested {
                            extension_id: extension.id.clone(),
                            source_id: extension.source_id.clone(),
                            display_name: display_name.clone(),
                            enabled,
                        });
                    }
                }
            });
        }
    }

    fn draw_uninstall_confirmation(
        &mut self,
        ui: &mut egui::Ui,
        event_tx: &Sender<UiEvent>,
        extension: &InstalledExtension,
        display_name: &str,
        busy: bool,
    ) {
        let theme = settings_theme(ui);
        if ui.add_enabled(!busy, danger_button(ui, "Remove")).clicked() {
            self.extension_busy = Some(extension_busy_key(&extension.id, &extension.source_id));
            self.set_status(format!("Uninstalling {display_name}"));
            let _ = event_tx.send(UiEvent::UninstallExtensionRequested {
                extension_id: extension.id.clone(),
                source_id: extension.source_id.clone(),
                display_name: display_name.to_string(),
            });
        }

        if ui
            .add_enabled(!busy, secondary_button_widget(ui, "Cancel"))
            .clicked()
        {
            self.pending_uninstall = None;
        }

        selectable_label(
            ui,
            settings_text(theme, "Remove from this device?", SettingsTextTone::Warning),
        );
    }

    fn installed_extension_display_name(&self, extension: &InstalledExtension) -> String {
        self.catalog
            .as_ref()
            .and_then(|catalog| {
                catalog
                    .entries
                    .iter()
                    .find(|entry| entry.id == extension.id)
                    .map(|entry| entry.name.clone())
            })
            .unwrap_or_else(|| extension.id.clone())
    }

    fn is_uninstall_pending(&self, extension_id: &str, source_id: &str) -> bool {
        self.pending_uninstall.as_ref().is_some_and(|pending| {
            pending.extension_id == extension_id && pending.source_id == source_id
        })
    }

    fn draw_catalog(&mut self, ui: &mut egui::Ui, event_tx: &Sender<UiEvent>) {
        let theme = settings_theme(ui);
        let Some(catalog) = &self.catalog else {
            empty_state(ui, "Refresh the catalog to browse available extensions.");
            return;
        };

        let installed_versions = installed_versions_by_id(&self.installed_state.extensions);
        let mut platform_entries = catalog
            .entries
            .iter()
            .filter(|entry| entry.platform == self.current_os)
            .collect::<Vec<_>>();
        platform_entries.sort_by(|left, right| left.name.cmp(&right.name));

        if platform_entries.is_empty() {
            empty_state(ui, "No extensions are available for this platform.");
            return;
        }

        ui.add_sized(
            [ui.available_width().min(TEXT_INPUT_WIDTH), 30.0],
            egui::TextEdit::singleline(&mut self.catalog_filter_text)
                .hint_text(egui::RichText::new("Search catalog").color(theme.text_muted)),
        );
        ui.add_space(10.0);

        let mut visible_entries =
            filter_catalog_entries(platform_entries, self.catalog_filter_text.trim())
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
        visible_entries.sort_by(|left, right| left.name.cmp(&right.name));

        if visible_entries.is_empty() {
            empty_state(ui, "No catalog extensions match your search.");
            return;
        }

        egui::ScrollArea::vertical()
            .min_scrolled_height(CATALOG_MIN_HEIGHT)
            .max_height(CATALOG_MAX_HEIGHT)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for entry in visible_entries {
                    let installed_version = installed_versions.get(&entry.id);
                    self.draw_catalog_entry(
                        ui,
                        event_tx,
                        &entry,
                        installed_version.map(String::as_str),
                    );
                }
            });
    }

    fn draw_catalog_entry(
        &mut self,
        ui: &mut egui::Ui,
        event_tx: &Sender<UiEvent>,
        entry: &CatalogEntry,
        installed_version: Option<&str>,
    ) {
        let theme = settings_theme(ui);
        list_row(ui, |ui| {
            ui.vertical(|ui| {
                extension_title_with_version(ui, &entry.name, &entry.version);
                if let Some(version) = installed_version {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        if version == entry.version {
                            badge(ui, "Installed", theme.success);
                        } else {
                            badge(ui, "Update available", theme.warning);
                        }
                    });
                }
                if let Some(description) = &entry.description {
                    let spacing = if installed_version.is_some() {
                        4.0
                    } else {
                        2.0
                    };
                    ui.add_space(spacing);
                    selectable_label(
                        ui,
                        settings_text(theme, description, SettingsTextTone::Muted),
                    );
                }
            });

            right_aligned_actions(ui, |ui| {
                if entry.kind != ExtensionKind::Static {
                    badge(ui, "Unavailable", theme.warning);
                    return;
                }

                let busy_key = extension_busy_key(&entry.id, GITHUB_SOURCE_ID);
                let busy = self.extension_busy.as_deref() == Some(busy_key.as_str());
                let label = match installed_version {
                    Some(version) if version == entry.version => "Reinstall",
                    Some(_) => "Update",
                    None => "Install",
                };

                if installed_version.is_some() {
                    if self.is_uninstall_pending(&entry.id, GITHUB_SOURCE_ID) {
                        if ui.add_enabled(!busy, danger_button(ui, "Remove")).clicked() {
                            self.extension_busy = Some(busy_key.clone());
                            self.set_status(format!("Uninstalling {}", entry.name));
                            let _ = event_tx.send(UiEvent::UninstallExtensionRequested {
                                extension_id: entry.id.clone(),
                                source_id: GITHUB_SOURCE_ID.to_string(),
                                display_name: entry.name.clone(),
                            });
                        }

                        if ui
                            .add_enabled(!busy, secondary_button_widget(ui, "Cancel"))
                            .clicked()
                        {
                            self.pending_uninstall = None;
                        }

                        selectable_label(
                            ui,
                            settings_text(
                                theme,
                                "Remove from this device?",
                                SettingsTextTone::Warning,
                            ),
                        );
                        return;
                    }

                    if ui
                        .add_enabled(!busy, danger_button(ui, "Uninstall"))
                        .clicked()
                    {
                        self.pending_uninstall = Some(PendingUninstall {
                            extension_id: entry.id.clone(),
                            source_id: GITHUB_SOURCE_ID.to_string(),
                        });
                    }
                }

                let action_button = if label == "Reinstall" {
                    secondary_button_widget(ui, label)
                } else {
                    primary_button(ui, label)
                };
                if ui.add_enabled(!busy, action_button).clicked() {
                    self.extension_busy = Some(busy_key);
                    self.set_status(format!("Installing {}", entry.name));
                    let _ = event_tx.send(UiEvent::InstallExtensionRequested {
                        source: self.draft.github.clone(),
                        entry: entry.clone(),
                        installed_version: installed_version.map(|version| version.to_string()),
                    });
                }
            });
        });
    }

    fn draw_extension_settings_panel(&mut self, ctx: &egui::Context, event_tx: &Sender<UiEvent>) {
        let Some(mut panel) = self.extension_settings_panel.take() else {
            return;
        };

        let mut open = true;
        let mut close_requested = false;
        let mut save_requested = false;
        let mut reset_requested = false;
        let title = format!("{} Settings", panel.target.display_name);
        let theme = settings_theme_from_ctx(ctx);
        let available_size = ctx.content_rect().size();
        let max_width = (available_size.x - 96.0).clamp(360.0, 720.0);
        let max_height = (available_size.y - 96.0).clamp(320.0, 560.0);
        let window_frame = egui::Frame::window(&ctx.global_style())
            .fill(theme.bg)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin {
                left: 18,
                right: 18,
                top: 16,
                bottom: 16,
            });

        egui::Window::new(title)
            .id(egui::Id::new(panel.target.key()))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(560.0, 440.0))
            .max_width(max_width)
            .max_height(max_height)
            .frame(window_frame)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                apply_settings_visuals(ui);
                selectable_label(
                    ui,
                    settings_text(
                        theme,
                        "These settings affect only this extension and are saved in your user extension folder.",
                        SettingsTextTone::Muted,
                    )
                );
                ui.add_space(16.0);

                if panel.schema.items.is_empty() {
                    empty_state(
                        ui,
                        "No custom settings are currently available for this extension.",
                    );
                } else {
                    let body_max_height = (max_height - 146.0).max(120.0);
                    egui::ScrollArea::vertical()
                        .max_height(body_max_height)
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            for category in rendered_settings_categories(&panel.schema) {
                                draw_settings_category(ui, &mut panel, category);
                            }
                        });
                }

                ui.add_space(12.0);
                save_bar(ui, panel.is_dirty(), panel.saving, |ui| {
                    if ui
                        .add_enabled(!panel.saving, secondary_button_widget(ui, "Reset Defaults"))
                        .clicked()
                    {
                        reset_requested = true;
                    }

                    if ui
                        .add_enabled(
                            panel.is_dirty() && !panel.saving,
                            primary_button(ui, "Save Settings"),
                        )
                        .clicked()
                    {
                        panel.saving = true;
                        save_requested = true;
                    }

                    if ui
                        .add_enabled(!panel.saving, secondary_button_widget(ui, "Close"))
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });

        if reset_requested {
            panel.draft_values = default_extension_settings_values(&panel.schema);
        }

        if save_requested {
            let _ = event_tx.send(UiEvent::SaveExtensionSettingsRequested {
                target: panel.target.clone(),
                values: panel.draft_values.clone(),
            });
        }

        if open && !close_requested {
            self.extension_settings_panel = Some(panel);
        }
    }
}

pub fn show_settings_viewport(
    ctx: &egui::Context,
    settings: std::sync::Arc<std::sync::Mutex<SettingsState>>,
    event_tx: Sender<UiEvent>,
) {
    ctx.show_viewport_deferred(
        settings_viewport_id(),
        egui::ViewportBuilder::default()
            .with_title("Omni Palette Settings")
            .with_inner_size([SETTINGS_WIDTH, SETTINGS_HEIGHT])
            .with_min_inner_size([760.0, 520.0])
            .with_resizable(true),
        move |ui, _class| {
            if let Ok(mut settings) = settings.lock() {
                if settings.take_focus_request() {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                settings.draw(ui, &event_tx);
            }
        },
    );
}

#[derive(Debug, Clone, Copy)]
enum BannerTone {
    Info,
    Warning,
    Error,
}

fn settings_theme(ui: &egui::Ui) -> SettingsTheme {
    current_app_theme(ui.ctx()).settings
}

fn settings_theme_from_ctx(ctx: &egui::Context) -> SettingsTheme {
    current_app_theme(ctx).settings
}

fn apply_settings_visuals(ui: &mut egui::Ui) {
    let theme = settings_theme(ui);
    let visuals = ui.visuals_mut();
    visuals.override_text_color = Some(theme.text_primary);
    visuals.panel_fill = theme.bg;
    visuals.window_fill = theme.bg;
    visuals.extreme_bg_color = theme.input_bg;
    visuals.faint_bg_color = theme.surface;
    visuals.selection.bg_fill = theme.accent_soft;
    visuals.selection.stroke = egui::Stroke::new(1.0, theme.accent);
    visuals.text_cursor.stroke = egui::Stroke::new(2.0, theme.accent);

    visuals.widgets.noninteractive.bg_fill = theme.surface;
    visuals.widgets.noninteractive.fg_stroke.color = theme.text_secondary;
    visuals.widgets.inactive.bg_fill = theme.input_bg;
    visuals.widgets.inactive.weak_bg_fill = theme.surface_alt;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, theme.border);
    visuals.widgets.inactive.fg_stroke.color = theme.text_primary;
    visuals.widgets.hovered.bg_fill = theme.input_hover;
    visuals.widgets.hovered.weak_bg_fill = theme.input_hover;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, theme.accent);
    visuals.widgets.hovered.fg_stroke.color = theme.text_primary;
    visuals.widgets.active.bg_fill = theme.accent_soft;
    visuals.widgets.active.weak_bg_fill = theme.accent_soft;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, theme.accent);
    visuals.widgets.active.fg_stroke.color = theme.text_on_accent;

    let style = ui.style_mut();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTextTone {
    Primary,
    Secondary,
    Muted,
    Warning,
    Error,
}

fn settings_text_color(theme: SettingsTheme, tone: SettingsTextTone) -> egui::Color32 {
    match tone {
        SettingsTextTone::Primary => theme.text_primary,
        SettingsTextTone::Secondary => theme.text_secondary,
        SettingsTextTone::Muted => theme.text_muted,
        SettingsTextTone::Warning => theme.warning,
        SettingsTextTone::Error => theme.error,
    }
}

fn settings_text(
    theme: SettingsTheme,
    text: impl Into<String>,
    tone: SettingsTextTone,
) -> egui::RichText {
    egui::RichText::new(text.into()).color(settings_text_color(theme, tone))
}

fn selectable_label(ui: &mut egui::Ui, text: egui::RichText) -> egui::Response {
    ui.add(
        egui::Label::new(text)
            .selectable(true)
            .sense(egui::Sense::CLICK | egui::Sense::DRAG),
    )
}

fn page_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    let theme = settings_theme(ui);
    selectable_label(
        ui,
        settings_text(theme, title, SettingsTextTone::Primary)
            .size(28.0)
            .strong(),
    );
    ui.add_space(2.0);
    selectable_label(
        ui,
        settings_text(theme, subtitle, SettingsTextTone::Muted).size(14.0),
    );
    ui.add_space(20.0);
}

fn section(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let theme = settings_theme(ui);
    egui::Frame::new()
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.border_soft))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin {
            left: 18,
            right: 18,
            top: 16,
            bottom: 16,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            selectable_label(
                ui,
                settings_text(theme, title, SettingsTextTone::Primary)
                    .size(16.5)
                    .strong(),
            );
            ui.add_space(2.0);
            selectable_label(
                ui,
                settings_text(theme, subtitle, SettingsTextTone::Muted).size(13.0),
            );
            ui.add_space(14.0);
            add_contents(ui);
        });
    ui.add_space(14.0);
}

#[derive(Debug, Clone)]
struct RenderedSettingsCategory {
    category: ExtensionSettingsCategory,
    items: Vec<ExtensionSettingItem>,
}

fn default_expanded_categories(schema: &ExtensionSettingsSchema) -> HashSet<String> {
    schema
        .categories
        .iter()
        .filter(|category| !category.default_collapsed)
        .map(|category| category.key.clone())
        .collect()
}

fn rendered_settings_categories(schema: &ExtensionSettingsSchema) -> Vec<RenderedSettingsCategory> {
    let mut items_by_category = HashMap::<String, Vec<ExtensionSettingItem>>::new();
    let mut general_items = Vec::new();

    for item in &schema.items {
        if let Some(category_key) = &item.category {
            items_by_category
                .entry(category_key.clone())
                .or_default()
                .push(item.clone());
        } else {
            general_items.push(item.clone());
        }
    }

    let mut categories = Vec::new();
    if !general_items.is_empty() {
        categories.push(RenderedSettingsCategory {
            category: ExtensionSettingsCategory {
                key: "__general__".to_string(),
                label: "General".to_string(),
                description: None,
                toggle_key: None,
                default_collapsed: false,
            },
            items: general_items,
        });
    }

    for category in &schema.categories {
        categories.push(RenderedSettingsCategory {
            category: category.clone(),
            items: items_by_category.remove(&category.key).unwrap_or_default(),
        });
    }

    categories
}

fn default_extension_settings_values(schema: &ExtensionSettingsSchema) -> ExtensionSettingsValues {
    schema
        .items
        .iter()
        .map(|item| (item.key.clone(), item.default))
        .collect()
}

fn draw_settings_category(
    ui: &mut egui::Ui,
    panel: &mut ExtensionSettingsPanelState,
    category: RenderedSettingsCategory,
) {
    let theme = settings_theme(ui);
    let category_key = category.category.key.clone();
    let mut expanded = panel.expanded_categories.contains(&category_key);
    let toggle_item = category
        .category
        .toggle_key
        .as_ref()
        .and_then(|toggle_key| category.items.iter().find(|item| item.key == *toggle_key))
        .cloned();
    let mut header_clicked = false;

    egui::Frame::new()
        .fill(theme.surface_alt)
        .stroke(egui::Stroke::new(1.0, theme.border_soft))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin {
            left: 16,
            right: 16,
            top: 14,
            bottom: 14,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let caret = if expanded { "v" } else { ">" };
                let caret_response = ui.add(
                    egui::Button::new(egui::RichText::new(caret).color(theme.text_secondary))
                        .frame(false)
                        .min_size(egui::vec2(18.0, 18.0)),
                );
                let title_response = ui
                    .vertical(|ui| {
                        selectable_label(
                            ui,
                            settings_text(
                                theme,
                                &category.category.label,
                                SettingsTextTone::Primary,
                            )
                            .size(15.0)
                            .strong(),
                        );
                        if let Some(description) = &category.category.description {
                            ui.add_space(2.0);
                            selectable_label(
                                ui,
                                settings_text(theme, description, SettingsTextTone::Muted)
                                    .size(12.0),
                            );
                        }
                    })
                    .response
                    .interact(egui::Sense::click());

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(toggle_item) = &toggle_item {
                        draw_category_header_toggle(ui, panel, toggle_item);
                    }
                });

                header_clicked = caret_response.clicked() || title_response.clicked();
            });

            if header_clicked {
                expanded = !expanded;
            }

            if expanded {
                let hidden_toggle_key = category.category.toggle_key.as_deref();
                let child_items = category
                    .items
                    .into_iter()
                    .filter(|item| Some(item.key.as_str()) != hidden_toggle_key)
                    .collect::<Vec<_>>();

                if !child_items.is_empty() {
                    ui.add_space(10.0);
                    for item in child_items {
                        draw_toggle_setting_row(ui, panel, item);
                    }
                }
            }
        });

    if expanded {
        panel.expanded_categories.insert(category_key);
    } else {
        panel.expanded_categories.remove(&category_key);
    }
    ui.add_space(10.0);
}

fn draw_category_header_toggle(
    ui: &mut egui::Ui,
    panel: &mut ExtensionSettingsPanelState,
    item: &ExtensionSettingItem,
) {
    let theme = settings_theme(ui);
    let value = panel
        .draft_values
        .entry(item.key.clone())
        .or_insert(item.default);

    ui.add(toggle_switch::toggle(value));
    selectable_label(
        ui,
        settings_text(theme, &item.label, SettingsTextTone::Secondary).size(12.0),
    );
}

fn draw_toggle_setting_row(
    ui: &mut egui::Ui,
    panel: &mut ExtensionSettingsPanelState,
    item: ExtensionSettingItem,
) {
    let theme = settings_theme(ui);
    let value = panel
        .draft_values
        .entry(item.key.clone())
        .or_insert(item.default);

    egui::Frame::new()
        .fill(theme.input_bg)
        .stroke(egui::Stroke::new(1.0, theme.border_soft))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 10,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    selectable_label(
                        ui,
                        settings_text(theme, &item.label, SettingsTextTone::Primary),
                    );
                    if let Some(description) = item.description {
                        ui.add_space(4.0);
                        selectable_label(
                            ui,
                            settings_text(theme, description, SettingsTextTone::Muted).size(12.0),
                        );
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(toggle_switch::toggle(value));
                });
            });
        });
    ui.add_space(8.0);
}

fn setting_row(ui: &mut egui::Ui, label: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    let theme = settings_theme(ui);
    ui.horizontal_top(|ui| {
        ui.add_sized(
            [ROW_LABEL_WIDTH, 30.0],
            egui::Label::new(settings_text(theme, label, SettingsTextTone::Secondary))
                .selectable(true)
                .sense(egui::Sense::CLICK | egui::Sense::DRAG),
        );
        ui.horizontal_wrapped(|ui| {
            add_contents(ui);
        });
    });
    ui.add_space(8.0);
}

fn setting_row_with_help(
    ui: &mut egui::Ui,
    label: &str,
    help: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let theme = settings_theme(ui);
    ui.horizontal(|ui| {
        setting_help_label(ui, label, help, theme);
        add_contents(ui);
    });
    ui.add_space(8.0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HelpIconVisuals {
    fill: egui::Color32,
    stroke: egui::Color32,
    text: egui::Color32,
}

fn setting_help_label(ui: &mut egui::Ui, label: &str, help: &str, theme: SettingsTheme) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ROW_LABEL_WIDTH, SETTING_ROW_HEIGHT),
        egui::Sense::hover(),
    );

    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let label_galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), font_id.clone(), theme.text_secondary);
    let label_pos = egui::pos2(
        setting_help_label_left(rect, label_galley.size().x),
        rect.center().y - (label_galley.size().y / 2.0),
    );
    ui.painter()
        .galley(label_pos, label_galley.clone(), theme.text_secondary);

    let icon_rect = setting_help_icon_rect(rect, label_galley.size().x);
    let response = ui
        .interact(
            icon_rect,
            ui.id().with(("setting_help", label)),
            egui::Sense::hover(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(help);
    let visuals = setting_help_icon_visuals(theme, response.hovered());
    let center = icon_rect.center();
    let radius = HELP_ICON_DIAMETER / 2.0;

    ui.painter().circle_filled(center, radius, visuals.fill);
    ui.painter()
        .circle_stroke(center, radius, egui::Stroke::new(1.0, visuals.stroke));
    ui.painter().text(
        center,
        egui::Align2::CENTER_CENTER,
        "?",
        egui::FontId::proportional(10.5),
        visuals.text,
    );
}

fn setting_help_icon_rect(row_rect: egui::Rect, label_width: f32) -> egui::Rect {
    let left = (setting_help_label_left(row_rect, label_width) + label_width + HELP_ICON_GAP)
        .min(row_rect.right() - HELP_ICON_DIAMETER);
    egui::Rect::from_min_size(
        egui::pos2(left, row_rect.center().y - (HELP_ICON_DIAMETER / 2.0)),
        egui::vec2(HELP_ICON_DIAMETER, HELP_ICON_DIAMETER),
    )
}

fn setting_help_label_left(row_rect: egui::Rect, label_width: f32) -> f32 {
    let group_width = label_width + HELP_ICON_GAP + HELP_ICON_DIAMETER;
    row_rect.left() + ((row_rect.width() - group_width) / 2.0).max(0.0)
}

fn setting_help_icon_visuals(theme: SettingsTheme, hovered: bool) -> HelpIconVisuals {
    HelpIconVisuals {
        fill: if hovered {
            theme.input_hover
        } else {
            theme.surface_alt
        },
        stroke: if hovered { theme.accent } else { theme.border },
        text: if hovered {
            theme.accent
        } else {
            theme.text_muted
        },
    }
}

fn extension_source_status_row(
    ui: &mut egui::Ui,
    enabled: &mut bool,
    controls_enabled: bool,
) -> (egui::Response, egui::Rect, egui::Rect, egui::Rect) {
    let theme = settings_theme(ui);
    let mut layout = None;

    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), SETTING_ROW_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = EXTENSION_SOURCE_ROW_SPACING;
            let toggle = ui
                .add_enabled_ui(controls_enabled, |ui| {
                    ui.add(toggle_switch::toggle(enabled))
                })
                .inner;
            let label = ui.label(
                egui::RichText::new("Enable remote extension catalog").color(theme.text_primary),
            );
            let state = if *enabled {
                ("Enabled", theme.success)
            } else {
                ("Disabled", theme.text_muted)
            };
            let badge = badge(ui, state.0, state.1);

            layout = Some((toggle, label.rect, badge.rect));
        },
    );

    let (toggle, label_rect, badge_rect) =
        layout.expect("extension source row should always render controls");
    let toggle_rect = toggle.rect;
    (toggle, toggle_rect, label_rect, badge_rect)
}

fn radio_option_row(ui: &mut egui::Ui, add_options: impl FnOnce(&mut egui::Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = RADIO_OPTION_SPACING;
        ui.horizontal(|ui| {
            add_options(ui);
        });
    });
}

fn save_bar(ui: &mut egui::Ui, dirty: bool, saving: bool, add_actions: impl FnOnce(&mut egui::Ui)) {
    let theme = settings_theme(ui);
    let (text, color) = if saving {
        ("Saving settings...", theme.accent)
    } else if dirty {
        ("Unsaved changes", theme.warning)
    } else {
        ("All changes saved", theme.success)
    };

    egui::Frame::new()
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.border_soft))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin {
            left: 16,
            right: 16,
            top: 12,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                badge(ui, text, color);
                right_aligned_actions(ui, |ui| {
                    add_actions(ui);
                });
            });
        });
}

fn nav_button(ui: &mut egui::Ui, active: bool, title: &str, subtitle: &str) -> egui::Response {
    let theme = settings_theme(ui);
    let desired_size = egui::vec2(ui.available_width(), 52.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    let hovered = response.hovered();
    let fill = if active {
        theme.nav_selected
    } else if hovered {
        theme.surface_alt
    } else {
        egui::Color32::TRANSPARENT
    };
    let stroke = if active {
        egui::Stroke::new(1.0, theme.accent)
    } else {
        egui::Stroke::NONE
    };

    ui.painter().rect(
        rect,
        egui::CornerRadius::same(10),
        fill,
        stroke,
        egui::StrokeKind::Outside,
    );

    let accent_rect = egui::Rect::from_min_size(
        rect.left_center() - egui::vec2(0.0, 15.0),
        egui::vec2(3.0, 30.0),
    );
    if active {
        ui.painter()
            .rect_filled(accent_rect, egui::CornerRadius::same(2), theme.accent);
    }

    ui.painter().text(
        rect.left_top() + egui::vec2(14.0, 10.0),
        egui::Align2::LEFT_TOP,
        title,
        egui::FontId::proportional(14.5),
        theme.text_primary,
    );
    ui.painter().text(
        rect.left_top() + egui::vec2(14.0, 30.0),
        egui::Align2::LEFT_TOP,
        subtitle,
        egui::FontId::proportional(11.5),
        theme.text_muted,
    );

    response.on_hover_text(subtitle)
}

fn primary_button<'a>(ui: &egui::Ui, label: &'a str) -> egui::Button<'a> {
    let theme = settings_theme(ui);
    egui::Button::new(
        egui::RichText::new(label)
            .strong()
            .color(theme.primary_button_text),
    )
    .fill(theme.primary_button_bg)
    .stroke(egui::Stroke::new(1.0, theme.primary_button_border))
    .corner_radius(egui::CornerRadius::same(7))
    .min_size(egui::vec2(104.0, 32.0))
}

fn danger_button<'a>(ui: &egui::Ui, label: &'a str) -> egui::Button<'a> {
    let theme = settings_theme(ui);
    egui::Button::new(
        egui::RichText::new(label)
            .strong()
            .color(theme.danger_button_text),
    )
    .fill(theme.danger_button_bg)
    .stroke(egui::Stroke::new(1.0, theme.danger_button_border))
    .corner_radius(egui::CornerRadius::same(7))
    .min_size(egui::vec2(88.0, 32.0))
}

fn extension_enabled_label(enabled: bool) -> &'static str {
    if enabled {
        "Enabled"
    } else {
        "Disabled"
    }
}

fn extension_kind_badge(kind: ExtensionKind) -> &'static str {
    match kind {
        ExtensionKind::Static => "Static",
        ExtensionKind::WasmPlugin => "Plugin",
    }
}

fn secondary_button_widget<'a>(ui: &egui::Ui, label: &'a str) -> egui::Button<'a> {
    let theme = settings_theme(ui);
    egui::Button::new(egui::RichText::new(label).color(theme.text_primary))
        .fill(theme.surface_alt)
        .stroke(egui::Stroke::new(1.0, theme.border))
        .corner_radius(egui::CornerRadius::same(7))
        .min_size(egui::vec2(88.0, 32.0))
}

fn secondary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(secondary_button_widget(ui, label))
}

fn action_button_row(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = ACTION_BUTTON_SPACING;
        ui.horizontal(|ui| add_contents(ui));
    });
}

fn right_aligned_actions(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = ACTION_BUTTON_SPACING;
        add_contents(ui);
    });
}

fn extension_enabled_actions(
    ui: &mut egui::Ui,
    saved_enabled: bool,
    enabled: &mut bool,
    controls_enabled: bool,
    add_actions: impl FnOnce(&mut egui::Ui),
) {
    let theme = settings_theme(ui);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(
                extension_action_area_width(ui),
                SETTING_ROW_HEIGHT.max(32.0),
            ),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = ACTION_BUTTON_SPACING;
                ui.allocate_ui_with_layout(
                    egui::vec2(EXTENSION_STATUS_LABEL_WIDTH, SETTING_ROW_HEIGHT),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.label(
                            egui::RichText::new(extension_enabled_label(saved_enabled))
                                .color(theme.text_primary),
                        );
                    },
                );
                ui.add_enabled_ui(controls_enabled, |ui| {
                    ui.add(toggle_switch::toggle(enabled));
                });
                ui.allocate_ui_with_layout(
                    egui::vec2(EXTENSION_ACTION_SLOT_WIDTH, SETTING_ROW_HEIGHT.max(32.0)),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = ACTION_BUTTON_SPACING;
                        add_actions(ui);
                    },
                );
            },
        );
    });
}

fn extension_action_area_width(ui: &egui::Ui) -> f32 {
    extension_action_area_width_for_toggle(ui.spacing().interact_size.y * 1.85)
}

fn extension_action_area_width_for_toggle(toggle_width: f32) -> f32 {
    EXTENSION_STATUS_LABEL_WIDTH
        + toggle_width
        + EXTENSION_ACTION_SLOT_WIDTH
        + (ACTION_BUTTON_SPACING * 2.0)
}

fn shortcut_pill(ui: &mut egui::Ui, text: &str) {
    let theme = settings_theme(ui);
    egui::Frame::new()
        .fill(theme.input_bg)
        .stroke(egui::Stroke::new(1.0, theme.border))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 7,
            bottom: 7,
        })
        .show(ui, |ui| {
            selectable_label(
                ui,
                settings_text(theme, text, SettingsTextTone::Primary)
                    .font(egui::FontId::monospace(14.0)),
            );
        });
}

fn extension_title_with_version(ui: &mut egui::Ui, name: &str, version: &str) {
    let theme = settings_theme(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        selectable_label(
            ui,
            settings_text(theme, name, SettingsTextTone::Primary)
                .size(15.0)
                .strong(),
        );
        selectable_label(
            ui,
            settings_text(theme, format!("v{version}"), SettingsTextTone::Muted).size(12.0),
        );
    });
}

fn path_chip(ui: &mut egui::Ui, text: &str) {
    let theme = settings_theme(ui);
    egui::Frame::new()
        .fill(theme.input_bg)
        .stroke(egui::Stroke::new(1.0, theme.border_soft))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(TEXT_INPUT_WIDTH));
            selectable_label(
                ui,
                settings_text(theme, text, SettingsTextTone::Secondary)
                    .font(egui::FontId::monospace(13.0)),
            );
        });
}

fn banner(ui: &mut egui::Ui, message: &str, tone: BannerTone) {
    let theme = settings_theme(ui);
    let (fill, stroke, text_tone) = match tone {
        BannerTone::Info => (
            theme.info_bg,
            theme.accent_soft,
            SettingsTextTone::Secondary,
        ),
        BannerTone::Warning => (theme.warning_bg, theme.warning, SettingsTextTone::Warning),
        BannerTone::Error => (theme.error_bg, theme.error, SettingsTextTone::Error),
    };

    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .corner_radius(egui::CornerRadius::same(9))
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 9,
            bottom: 9,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            selectable_label(ui, settings_text(theme, message, text_tone));
        });
}

fn os_label(os: Os) -> &'static str {
    match os {
        Os::Windows => "Windows",
        Os::Mac => "macOS",
        Os::Linux => "Linux",
    }
}

fn toast(ui: &mut egui::Ui, message: &str) {
    let theme = settings_theme(ui);
    egui::Frame::new()
        .fill(theme.info_bg)
        .stroke(egui::Stroke::new(1.0, theme.accent_soft))
        .corner_radius(egui::CornerRadius::same(10))
        .shadow(egui::Shadow {
            offset: [0, 10],
            blur: 24,
            spread: 0,
            color: theme.shadow,
        })
        .inner_margin(egui::Margin {
            left: 14,
            right: 14,
            top: 10,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_max_width(360.0);
            selectable_label(
                ui,
                settings_text(theme, message, SettingsTextTone::Secondary),
            );
        });
}

fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) -> egui::Response {
    let theme = settings_theme(ui);
    egui::Frame::new()
        .fill(theme.input_bg)
        .stroke(egui::Stroke::new(1.0, theme.border_soft))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin {
            left: 9,
            right: 9,
            top: 4,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.5).color(color));
        })
        .response
}

fn list_row(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let theme = settings_theme(ui);
    egui::Frame::new()
        .fill(theme.surface_alt)
        .stroke(egui::Stroke::new(1.0, theme.border_soft))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin {
            left: 14,
            right: 14,
            top: 12,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                add_contents(ui);
            });
        });
    ui.add_space(8.0);
}

fn empty_state(ui: &mut egui::Ui, message: &str) {
    let theme = settings_theme(ui);
    egui::Frame::new()
        .fill(theme.input_bg)
        .stroke(egui::Stroke::new(1.0, theme.border_soft))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            selectable_label(ui, settings_text(theme, message, SettingsTextTone::Muted));
        });
}

fn edit_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    setting_row(ui, label, |ui| {
        let width = ui.available_width().clamp(240.0, TEXT_INPUT_WIDTH);
        ui.add_sized(
            [width, 30.0],
            egui::TextEdit::singleline(value).font(egui::TextStyle::Monospace),
        );
    });
}

fn validate_catalog_source(source: &GitHubExtensionSource) -> Result<(), String> {
    let mut missing = Vec::new();
    if source.owner.trim().is_empty() {
        missing.push("owner");
    }
    if source.repo.trim().is_empty() {
        missing.push("repo");
    }
    if source.branch.trim().is_empty() {
        missing.push("branch");
    }
    if source.catalog_path.trim().is_empty() {
        missing.push("catalog path");
    }

    if !missing.is_empty() {
        return Err(format!(
            "Catalog source is incomplete. Fill in: {}.",
            missing.join(", ")
        ));
    }

    Ok(())
}

fn filter_catalog_entries<'a>(
    entries: Vec<&'a CatalogEntry>,
    query: &str,
) -> Vec<&'a CatalogEntry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return entries;
    }

    entries
        .into_iter()
        .filter(|entry| catalog_entry_matches_query(entry, &query))
        .collect()
}

fn catalog_entry_matches_query(entry: &CatalogEntry, query: &str) -> bool {
    entry.name.to_lowercase().contains(query)
        || entry.id.to_lowercase().contains(query)
        || entry
            .description
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains(query)
        || entry
            .keywords
            .iter()
            .any(|keyword| keyword.to_lowercase().contains(query))
}

fn installed_versions_by_id(extensions: &[InstalledExtension]) -> HashMap<String, String> {
    extensions
        .iter()
        .filter(|extension| extension.source_id == GITHUB_SOURCE_ID)
        .map(|extension| (extension.id.clone(), extension.version.clone()))
        .collect()
}

fn extension_busy_key(extension_id: &str, source_id: &str) -> String {
    format!("{source_id}/{extension_id}")
}

fn capture_shortcut(ctx: &egui::Context) -> Option<KeyboardShortcut> {
    ctx.input(|input| {
        input.events.iter().find_map(|event| {
            let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            else {
                return None;
            };

            map_egui_key(*key).map(|key| KeyboardShortcut {
                key,
                modifier: HotkeyModifiers {
                    control: modifiers.ctrl,
                    shift: modifiers.shift,
                    alt: modifiers.alt,
                    win: false,
                },
            })
        })
    })
}

fn map_egui_key(key: egui::Key) -> Option<Key> {
    Some(match key {
        egui::Key::A => Key::KeyA,
        egui::Key::B => Key::KeyB,
        egui::Key::C => Key::KeyC,
        egui::Key::D => Key::KeyD,
        egui::Key::E => Key::KeyE,
        egui::Key::F => Key::KeyF,
        egui::Key::G => Key::KeyG,
        egui::Key::H => Key::KeyH,
        egui::Key::I => Key::KeyI,
        egui::Key::J => Key::KeyJ,
        egui::Key::K => Key::KeyK,
        egui::Key::L => Key::KeyL,
        egui::Key::M => Key::KeyM,
        egui::Key::N => Key::KeyN,
        egui::Key::O => Key::KeyO,
        egui::Key::P => Key::KeyP,
        egui::Key::Q => Key::KeyQ,
        egui::Key::R => Key::KeyR,
        egui::Key::S => Key::KeyS,
        egui::Key::T => Key::KeyT,
        egui::Key::U => Key::KeyU,
        egui::Key::V => Key::KeyV,
        egui::Key::W => Key::KeyW,
        egui::Key::X => Key::KeyX,
        egui::Key::Y => Key::KeyY,
        egui::Key::Z => Key::KeyZ,
        egui::Key::Num0 => Key::Key0,
        egui::Key::Num1 => Key::Key1,
        egui::Key::Num2 => Key::Key2,
        egui::Key::Num3 => Key::Key3,
        egui::Key::Num4 => Key::Key4,
        egui::Key::Num5 => Key::Key5,
        egui::Key::Num6 => Key::Key6,
        egui::Key::Num7 => Key::Key7,
        egui::Key::Num8 => Key::Key8,
        egui::Key::Num9 => Key::Key9,
        egui::Key::F1 => Key::F1,
        egui::Key::F2 => Key::F2,
        egui::Key::F3 => Key::F3,
        egui::Key::F4 => Key::F4,
        egui::Key::F5 => Key::F5,
        egui::Key::F6 => Key::F6,
        egui::Key::F7 => Key::F7,
        egui::Key::F8 => Key::F8,
        egui::Key::F9 => Key::F9,
        egui::Key::F10 => Key::F10,
        egui::Key::F11 => Key::F11,
        egui::Key::F12 => Key::F12,
        egui::Key::Semicolon | egui::Key::Colon => Key::Semicolon,
        egui::Key::Equals | egui::Key::Plus => Key::Equal,
        egui::Key::Comma => Key::Comma,
        egui::Key::Minus => Key::Minus,
        egui::Key::Period => Key::Period,
        egui::Key::Slash | egui::Key::Questionmark => Key::Slash,
        egui::Key::Backtick => Key::Grave,
        egui::Key::OpenBracket | egui::Key::OpenCurlyBracket => Key::LeftBracket,
        egui::Key::Backslash | egui::Key::Pipe => Key::Backslash,
        egui::Key::CloseBracket | egui::Key::CloseCurlyBracket => Key::RightBracket,
        egui::Key::Quote => Key::Apostrophe,
        egui::Key::Enter => Key::Enter,
        egui::Key::Space => Key::Space,
        egui::Key::Tab => Key::Tab,
        egui::Key::Escape => Key::Escape,
        egui::Key::Delete => Key::Delete,
        egui::Key::Backspace => Key::BackSpace,
        egui::Key::Home => Key::Home,
        egui::Key::End => Key::End,
        egui::Key::PageUp => Key::PageUp,
        egui::Key::PageDown => Key::PageDown,
        egui::Key::Insert => Key::Insert,
        egui::Key::ArrowLeft => Key::LeftArrow,
        egui::Key::ArrowRight => Key::RightArrow,
        egui::Key::ArrowUp => Key::UpArrow,
        egui::Key::ArrowDown => Key::DownArrow,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        default_expanded_categories, extension_action_area_width_for_toggle,
        extension_source_status_row, filter_catalog_entries, installed_versions_by_id,
        radio_option_row, rendered_settings_categories, setting_help_icon_rect,
        setting_help_icon_visuals, setting_row_with_help, settings_text_color,
        validate_catalog_source, SettingsBootstrap, SettingsState, SettingsTextTone,
        ACTION_BUTTON_SPACING, EXTENSION_ACTION_SLOT_WIDTH, EXTENSION_SOURCE_ROW_SPACING,
        EXTENSION_STATUS_LABEL_WIDTH, HELP_ICON_DIAMETER, HELP_ICON_GAP, RADIO_OPTION_SPACING,
        ROW_LABEL_WIDTH, SETTING_ROW_HEIGHT,
    };
    use crate::config::runtime::{GitHubExtensionSource, RuntimeConfig};
    use crate::core::extensions::catalog::{CatalogEntry, ExtensionKind};
    use crate::core::extensions::install::{
        InstalledExtension, InstalledState, BUNDLED_SOURCE_ID, GITHUB_SOURCE_ID,
    };
    use crate::core::extensions::settings::{
        ExtensionSettingItem, ExtensionSettingKind, ExtensionSettingsCategory,
        ExtensionSettingsSchema,
    };
    use crate::domain::action::Os;
    use crate::theme::SettingsTheme;
    use crate::ui::app::UiEvent;
    use std::cell::Cell;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::mpsc;

    fn valid_source() -> GitHubExtensionSource {
        GitHubExtensionSource {
            owner: "Greg-Lim".to_string(),
            repo: "omni-palette-desktop".to_string(),
            branch: "master".to_string(),
            catalog_path: "extensions/registry/catalog.v1.json".to_string(),
            enabled: true,
        }
    }

    fn settings_state(config: RuntimeConfig) -> SettingsState {
        SettingsState::new(SettingsBootstrap {
            config,
            config_path: Some(PathBuf::from("config.toml")),
            config_error: None,
            current_os: Os::Windows,
            install_root: Some(PathBuf::from("extensions")),
            bundled_extensions: Vec::new(),
            extension_settings_available: HashSet::new(),
            installed_state: InstalledState::default(),
            installed_state_error: None,
        })
    }

    #[test]
    fn opening_settings_requests_viewport_focus_once_per_open_request() {
        let mut state = settings_state(RuntimeConfig::default());

        assert!(!state.open);
        assert!(!state.take_focus_request());

        state.open();

        assert!(state.open);
        assert!(state.take_focus_request());
        assert!(!state.take_focus_request());

        state.open();

        assert!(state.open);
        assert!(state.take_focus_request());
        assert!(!state.take_focus_request());
    }

    #[test]
    fn catalog_source_validation_accepts_complete_unsigned_source() {
        let source = valid_source();

        validate_catalog_source(&source).expect("complete unsigned source should pass");
    }

    #[test]
    fn catalog_source_validation_reports_missing_required_fields() {
        let mut source = valid_source();
        source.owner.clear();
        source.branch = "   ".to_string();

        let err = validate_catalog_source(&source).expect_err("missing fields should fail");

        assert!(err.contains("owner"));
        assert!(err.contains("branch"));
    }

    fn catalog_entry(
        id: &str,
        name: &str,
        description: Option<&str>,
        keywords: &[&str],
    ) -> CatalogEntry {
        CatalogEntry {
            id: id.to_string(),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            platform: Os::Windows,
            kind: ExtensionKind::Static,
            package_url: format!(
                "https://github.com/Greg-Lim/omni-palette-desktop/releases/download/{id}-v1/{id}.gpext"
            ),
            package_sha256: "a".repeat(64),
            size_bytes: None,
            publisher: None,
            description: description.map(str::to_string),
            license: None,
            homepage: None,
            repository: None,
            keywords: keywords.iter().map(|keyword| keyword.to_string()).collect(),
            min_app_version: None,
        }
    }

    #[test]
    fn catalog_filter_empty_query_returns_all_entries() {
        let entries = vec![
            catalog_entry("alpha_tools", "Alpha Tools", None, &[]),
            catalog_entry("beta_tools", "Beta Tools", None, &[]),
        ];
        let refs = entries.iter().collect::<Vec<_>>();

        let filtered = filter_catalog_entries(refs, "  ");

        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn catalog_filter_matches_name_id_description_and_keywords() {
        let entries = vec![
            catalog_entry(
                "chrome",
                "Chrome",
                Some("Chrome keyboard shortcut command pack"),
                &["browser"],
            ),
            catalog_entry(
                "file_explorer",
                "File Explorer",
                Some("Manage folders"),
                &["files"],
            ),
        ];

        let by_name = filter_catalog_entries(entries.iter().collect(), "chrome");
        let by_id = filter_catalog_entries(entries.iter().collect(), "file_explorer");
        let by_description = filter_catalog_entries(entries.iter().collect(), "folders");
        let by_keyword = filter_catalog_entries(entries.iter().collect(), "files");

        assert_eq!(by_name[0].id, "chrome");
        assert_eq!(by_id[0].id, "file_explorer");
        assert_eq!(by_description[0].id, "file_explorer");
        assert_eq!(by_keyword[0].id, "file_explorer");
    }

    #[test]
    fn catalog_filter_is_case_insensitive() {
        let entries = vec![catalog_entry("chrome", "Chrome", None, &[])];

        let filtered = filter_catalog_entries(entries.iter().collect(), "CHROME");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "chrome");
    }

    #[test]
    fn catalog_source_toggle_change_auto_saves_runtime_config() {
        let mut config = RuntimeConfig::default();
        config.github.enabled = false;
        let mut state = settings_state(config);
        let (tx, rx) = mpsc::channel();

        let previous_enabled = state.draft.github.enabled;
        state.draft.github.enabled = true;
        state.save_catalog_source_enabled_change(previous_enabled, &tx);

        assert!(state.saving);
        assert_eq!(
            state.status.as_ref().map(|status| status.message.as_str()),
            Some("Saving catalog source")
        );

        let event = rx.try_recv().expect("toggle change should send save event");
        let UiEvent::SaveRuntimeConfigRequested(saved_config) = event else {
            panic!("expected runtime config save event, got {event:?}");
        };
        assert!(saved_config.github.enabled);
    }

    #[test]
    fn pending_catalog_source_toggle_auto_save_does_not_show_unsaved_warning() {
        let mut config = RuntimeConfig::default();
        config.github.enabled = false;
        let mut state = settings_state(config);
        let (tx, _rx) = mpsc::channel();

        let previous_enabled = state.draft.github.enabled;
        state.draft.github.enabled = true;
        state.save_catalog_source_enabled_change(previous_enabled, &tx);

        assert!(state.is_dirty());
        assert!(!state.has_visible_unsaved_settings_changes());
    }

    #[test]
    fn failed_catalog_source_toggle_auto_save_reverts_toggle_and_reports_error() {
        let mut config = RuntimeConfig::default();
        config.github.enabled = false;
        let mut state = settings_state(config);
        let (tx, _rx) = mpsc::channel();

        let previous_enabled = state.draft.github.enabled;
        state.draft.github.enabled = true;
        state.save_catalog_source_enabled_change(previous_enabled, &tx);
        state.config_saved(
            state.draft.clone(),
            Err("Could not save config".to_string()),
        );

        assert!(!state.saving);
        assert!(!state.draft.github.enabled);
        assert!(!state.saved.github.enabled);
        assert_eq!(
            state.status.as_ref().map(|status| status.message.as_str()),
            Some("Could not save config")
        );
        assert!(!state.has_visible_unsaved_settings_changes());
    }

    #[test]
    fn catalog_filter_returns_empty_for_non_matching_query() {
        let entries = vec![catalog_entry("chrome", "Chrome", None, &[])];

        let filtered = filter_catalog_entries(entries.iter().collect(), "missing");

        assert!(filtered.is_empty());
    }

    #[test]
    fn installed_versions_by_id_ignores_bundled_entries() {
        let extensions = vec![
            InstalledExtension {
                id: "windows".to_string(),
                version: "0.1.0".to_string(),
                platform: Os::Windows,
                kind: ExtensionKind::Static,
                source_id: BUNDLED_SOURCE_ID.to_string(),
                package_sha256: "0".repeat(64),
                enabled: true,
                installed_path: PathBuf::from("static/windows.toml"),
            },
            InstalledExtension {
                id: "chrome".to_string(),
                version: "0.1.0".to_string(),
                platform: Os::Windows,
                kind: ExtensionKind::Static,
                source_id: GITHUB_SOURCE_ID.to_string(),
                package_sha256: "1".repeat(64),
                enabled: true,
                installed_path: PathBuf::from("static/chrome.toml"),
            },
        ];

        let versions = installed_versions_by_id(&extensions);

        assert!(!versions.contains_key("windows"));
        assert_eq!(versions.get("chrome").map(String::as_str), Some("0.1.0"));
    }

    #[test]
    fn extension_action_area_reserves_a_fixed_trailing_action_slot() {
        let toggle_width = 40.0;
        let expected_width = EXTENSION_STATUS_LABEL_WIDTH
            + ACTION_BUTTON_SPACING
            + toggle_width
            + ACTION_BUTTON_SPACING
            + EXTENSION_ACTION_SLOT_WIDTH;

        assert_eq!(
            extension_action_area_width_for_toggle(toggle_width),
            expected_width
        );
    }

    #[test]
    fn settings_text_tones_map_to_theme_tokens() {
        let theme = crate::theme::AppTheme::for_egui_theme(egui::Theme::Light).settings;

        assert_eq!(
            settings_text_color(theme, SettingsTextTone::Primary),
            theme.text_primary
        );
        assert_eq!(
            settings_text_color(theme, SettingsTextTone::Secondary),
            theme.text_secondary
        );
        assert_eq!(
            settings_text_color(theme, SettingsTextTone::Muted),
            theme.text_muted
        );
        assert_eq!(
            settings_text_color(theme, SettingsTextTone::Warning),
            theme.warning
        );
        assert_eq!(
            settings_text_color(theme, SettingsTextTone::Error),
            theme.error
        );
    }

    #[test]
    fn rendered_settings_categories_synthesizes_general_for_uncategorized_items() {
        let categories = rendered_settings_categories(&ExtensionSettingsSchema {
            categories: vec![ExtensionSettingsCategory {
                key: "script".to_string(),
                label: "Script".to_string(),
                description: None,
                toggle_key: None,
                default_collapsed: true,
            }],
            items: vec![
                ExtensionSettingItem {
                    key: "general.toggle".to_string(),
                    label: "General toggle".to_string(),
                    description: None,
                    category: None,
                    kind: ExtensionSettingKind::Toggle,
                    default: true,
                },
                ExtensionSettingItem {
                    key: "script.toggle".to_string(),
                    label: "Script toggle".to_string(),
                    description: None,
                    category: Some("script".to_string()),
                    kind: ExtensionSettingKind::Toggle,
                    default: false,
                },
            ],
        });

        assert_eq!(categories.len(), 2);
        assert_eq!(categories[0].category.label, "General");
        assert_eq!(categories[0].items.len(), 1);
        assert_eq!(categories[1].category.key, "script");
        assert_eq!(categories[1].items[0].key, "script.toggle");
    }

    #[test]
    fn default_expanded_categories_uses_default_collapsed_flag() {
        let expanded = default_expanded_categories(&ExtensionSettingsSchema {
            categories: vec![
                ExtensionSettingsCategory {
                    key: "open".to_string(),
                    label: "Open".to_string(),
                    description: None,
                    toggle_key: None,
                    default_collapsed: false,
                },
                ExtensionSettingsCategory {
                    key: "closed".to_string(),
                    label: "Closed".to_string(),
                    description: None,
                    toggle_key: None,
                    default_collapsed: true,
                },
            ],
            items: vec![],
        });

        assert!(expanded.contains("open"));
        assert!(!expanded.contains("closed"));
    }

    #[test]
    fn setting_row_with_help_reserves_label_column_before_controls() {
        let control_left = Cell::new(0.0);

        egui::__run_test_ui(|ui| {
            ui.set_width(600.0);
            setting_row_with_help(ui, "Theme", "Theme help", |ui| {
                control_left.set(ui.cursor().left());
                ui.label("System");
            });
        });

        assert!(
            control_left.get() >= ROW_LABEL_WIDTH,
            "control started at {}, before the reserved label width {}",
            control_left.get(),
            ROW_LABEL_WIDTH
        );
    }

    #[test]
    fn radio_option_row_adds_readable_spacing_between_options() {
        let option_gap = Cell::new(0.0);

        egui::__run_test_ui(|ui| {
            ui.set_width(600.0);
            radio_option_row(ui, |ui| {
                let first = ui.label("System");
                let second = ui.label("Light");
                option_gap.set(second.rect.left() - first.rect.right());
            });
        });

        assert_eq!(option_gap.get(), RADIO_OPTION_SPACING);
    }

    #[test]
    fn extension_source_status_row_spaces_toggle_label_and_badge() {
        let toggle_to_label_gap = Cell::new(0.0);
        let label_to_badge_gap = Cell::new(0.0);

        egui::__run_test_ui(|ui| {
            ui.set_width(600.0);
            let mut enabled = true;
            let (_, toggle_rect, label_rect, badge_rect) =
                extension_source_status_row(ui, &mut enabled, true);
            toggle_to_label_gap.set(label_rect.left() - toggle_rect.right());
            label_to_badge_gap.set(badge_rect.left() - label_rect.right());
        });

        assert!(
            (toggle_to_label_gap.get() - EXTENSION_SOURCE_ROW_SPACING).abs() < 0.01,
            "toggle-to-label gap was {}, expected {}",
            toggle_to_label_gap.get(),
            EXTENSION_SOURCE_ROW_SPACING
        );
        assert!(
            (label_to_badge_gap.get() - EXTENSION_SOURCE_ROW_SPACING).abs() < 0.01,
            "label-to-badge gap was {}, expected {}",
            label_to_badge_gap.get(),
            EXTENSION_SOURCE_ROW_SPACING
        );
    }

    #[test]
    fn extension_source_status_row_does_not_stretch_inside_tall_parent() {
        let badge_height = Cell::new(0.0);

        egui::__run_test_ui(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(600.0, 900.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let mut enabled = true;
                    let (_, _, _, badge_rect) = extension_source_status_row(ui, &mut enabled, true);
                    badge_height.set(badge_rect.height());
                },
            );
        });

        assert!(
            badge_height.get() <= SETTING_ROW_HEIGHT,
            "badge stretched to {}px inside a tall parent",
            badge_height.get()
        );
    }

    #[test]
    fn setting_help_icon_sits_after_label_text_inside_label_column() {
        let row_rect =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(ROW_LABEL_WIDTH, 30.0));
        let label_width = 42.0;
        let group_width = label_width + HELP_ICON_GAP + HELP_ICON_DIAMETER;
        let expected_icon_left = row_rect.left()
            + ((row_rect.width() - group_width) / 2.0)
            + label_width
            + HELP_ICON_GAP;

        let icon_rect = setting_help_icon_rect(row_rect, label_width);

        assert_eq!(icon_rect.width(), HELP_ICON_DIAMETER);
        assert_eq!(icon_rect.height(), HELP_ICON_DIAMETER);
        assert_eq!(icon_rect.left(), expected_icon_left);
        assert!(icon_rect.right() <= row_rect.right());
        assert_eq!(icon_rect.center().y, row_rect.center().y);
    }

    #[test]
    fn setting_help_icon_visuals_change_on_hover() {
        let theme = SettingsTheme {
            bg: egui::Color32::BLACK,
            sidebar_bg: egui::Color32::BLACK,
            surface: egui::Color32::from_rgb(1, 1, 1),
            surface_alt: egui::Color32::from_rgb(2, 2, 2),
            input_bg: egui::Color32::from_rgb(3, 3, 3),
            input_hover: egui::Color32::from_rgb(4, 4, 4),
            border: egui::Color32::from_rgb(5, 5, 5),
            border_soft: egui::Color32::from_rgb(6, 6, 6),
            accent: egui::Color32::from_rgb(7, 7, 7),
            accent_soft: egui::Color32::from_rgb(8, 8, 8),
            text_primary: egui::Color32::from_rgb(9, 9, 9),
            text_secondary: egui::Color32::from_rgb(10, 10, 10),
            text_muted: egui::Color32::from_rgb(11, 11, 11),
            text_on_accent: egui::Color32::from_rgb(12, 12, 12),
            warning: egui::Color32::from_rgb(13, 13, 13),
            error: egui::Color32::from_rgb(14, 14, 14),
            success: egui::Color32::from_rgb(15, 15, 15),
            nav_selected: egui::Color32::from_rgb(16, 16, 16),
            info_bg: egui::Color32::from_rgb(17, 17, 17),
            warning_bg: egui::Color32::from_rgb(18, 18, 18),
            error_bg: egui::Color32::from_rgb(19, 19, 19),
            primary_button_text: egui::Color32::from_rgb(20, 20, 20),
            primary_button_bg: egui::Color32::from_rgb(21, 21, 21),
            primary_button_border: egui::Color32::from_rgb(22, 22, 22),
            danger_button_text: egui::Color32::from_rgb(23, 23, 23),
            danger_button_bg: egui::Color32::from_rgb(24, 24, 24),
            danger_button_border: egui::Color32::from_rgb(25, 25, 25),
            shadow: egui::Color32::from_rgb(26, 26, 26),
        };

        let idle = setting_help_icon_visuals(theme, false);
        let hovered = setting_help_icon_visuals(theme, true);

        assert_ne!(idle.fill, hovered.fill);
        assert_eq!(hovered.fill, theme.input_hover);
        assert_eq!(hovered.text, theme.accent);
    }
}
