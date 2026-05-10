use crate::core::registry::registry::UnitAction;
use crate::domain::action::{CommandPriority, ContextRoot, FocusState};
use crate::platform::platform_interface::RawWindowHandleExt;
use crate::platform::windows::context::context::get_hwnd_from_raw;
use crate::ui::app::{PaletteWorkArea, UiEvent};
use eframe::egui;
use raw_window_handle::RawWindowHandle;
use std::sync::mpsc::Sender;

pub(crate) const MAX_DEBUG_BACKGROUND_WINDOWS: usize = 12;
pub(crate) const MAX_DEBUG_COMMAND_ROWS: usize = 8;

pub(crate) const DEBUG_OVERLAY_VIEWPORT_ID: &str = "omni_palette_debug_overlay";
const DEBUG_OVERLAY_WIDTH: f32 = 360.0;
const DEBUG_OVERLAY_HEIGHT: f32 = 560.0;
const DEBUG_OVERLAY_MARGIN: f32 = 10.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DebugWindowSummary {
    pub(crate) process_name: Option<String>,
    pub(crate) hwnd: Option<isize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DebugWindowRole {
    Foreground,
    Background,
}

impl DebugWindowRole {
    fn label(self) -> &'static str {
        match self {
            Self::Foreground => "foreground",
            Self::Background => "background",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DebugRawWindow {
    pub(crate) role: DebugWindowRole,
    pub(crate) process_name: Option<String>,
    pub(crate) hwnd: Option<isize>,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DebugCommandCandidate {
    pub(crate) focus_state: FocusState,
    pub(crate) priority: CommandPriority,
    pub(crate) favorite: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DebugCommandSummary {
    pub(crate) total: usize,
    pub(crate) focused: usize,
    pub(crate) background: usize,
    pub(crate) global: usize,
    pub(crate) favorites: usize,
    pub(crate) suppressed_priority: usize,
    pub(crate) low_priority: usize,
    pub(crate) medium_priority: usize,
    pub(crate) high_priority: usize,
}

impl DebugCommandSummary {
    pub(crate) fn from_candidates(
        candidates: impl IntoIterator<Item = DebugCommandCandidate>,
    ) -> Self {
        let mut summary = Self::default();
        for candidate in candidates {
            summary.total += 1;
            if candidate.favorite {
                summary.favorites += 1;
            }
            match candidate.focus_state {
                FocusState::Focused => summary.focused += 1,
                FocusState::Background => summary.background += 1,
                FocusState::Global => summary.global += 1,
            }
            match candidate.priority {
                CommandPriority::Suppressed => summary.suppressed_priority += 1,
                CommandPriority::Low => summary.low_priority += 1,
                CommandPriority::Medium => summary.medium_priority += 1,
                CommandPriority::High => summary.high_priority += 1,
            }
        }
        summary
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DebugSnapshot {
    pub(crate) foreground_window: Option<DebugWindowSummary>,
    pub(crate) background_windows: Vec<DebugWindowSummary>,
    pub(crate) background_total: usize,
    pub(crate) raw_windows: Vec<DebugRawWindow>,
    pub(crate) active_tags: Vec<String>,
    pub(crate) text_input_active: bool,
    pub(crate) ignored_process_name: Option<String>,
    pub(crate) command_summary: DebugCommandSummary,
    pub(crate) work_area: Option<PaletteWorkArea>,
}

impl DebugSnapshot {
    #[cfg(test)]
    pub(crate) fn from_parts(
        foreground_window: Option<DebugWindowSummary>,
        background_windows: Vec<DebugWindowSummary>,
        active_interaction: crate::domain::action::InteractionContext,
        ignored_process_name: Option<String>,
        command_summary: DebugCommandSummary,
    ) -> Self {
        let background_total = background_windows.len();
        let mut raw_windows = Vec::new();
        if let Some(window) = &foreground_window {
            raw_windows.push(DebugRawWindow {
                role: DebugWindowRole::Foreground,
                process_name: window.process_name.clone(),
                hwnd: window.hwnd,
                tags: active_interaction.tags.clone(),
            });
        }
        raw_windows.extend(background_windows.iter().map(|window| DebugRawWindow {
            role: DebugWindowRole::Background,
            process_name: window.process_name.clone(),
            hwnd: window.hwnd,
            tags: Vec::new(),
        }));

        Self {
            foreground_window,
            background_windows: background_windows
                .into_iter()
                .take(MAX_DEBUG_BACKGROUND_WINDOWS)
                .collect(),
            background_total,
            raw_windows,
            text_input_active: active_interaction.has_tag("ui.text_input"),
            active_tags: active_interaction.tags,
            ignored_process_name,
            command_summary,
            work_area: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DebugCommandRow {
    pub(crate) label: String,
    pub(crate) focus_state: FocusState,
    pub(crate) priority: CommandPriority,
    pub(crate) favorite: bool,
    pub(crate) score: i32,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DebugPaletteState {
    pub(crate) query: String,
    pub(crate) filtered_count: usize,
    pub(crate) top_rows: Vec<DebugCommandRow>,
}

pub(crate) fn snapshot_from_context(
    context_root: &ContextRoot,
    unit_actions: &[UnitAction],
    ignored_process_name: Option<String>,
    work_area: Option<PaletteWorkArea>,
) -> DebugSnapshot {
    let foreground_window = context_root.get_active().copied().map(window_summary);
    let background_total = context_root.bg_context.len();
    let foreground_tags = context_root.active_interaction.tags.clone();
    let mut raw_windows: Vec<DebugRawWindow> = context_root
        .fg_context
        .iter()
        .copied()
        .map(|handle| raw_window(handle, DebugWindowRole::Foreground, foreground_tags.clone()))
        .collect();
    raw_windows.extend(
        context_root
            .bg_context
            .iter()
            .copied()
            .map(|handle| raw_window(handle, DebugWindowRole::Background, Vec::new())),
    );
    let background_windows = context_root
        .bg_context
        .iter()
        .take(MAX_DEBUG_BACKGROUND_WINDOWS)
        .copied()
        .map(window_summary)
        .collect();
    let command_summary = DebugCommandSummary::from_candidates(unit_actions.iter().map(|action| {
        DebugCommandCandidate {
            focus_state: action.focus_state,
            priority: action.metadata.priority,
            favorite: action.metadata.favorite,
        }
    }));
    let active_interaction = context_root.active_interaction.clone();

    DebugSnapshot {
        foreground_window,
        background_windows,
        background_total,
        raw_windows,
        text_input_active: active_interaction.has_tag("ui.text_input"),
        active_tags: active_interaction.tags,
        ignored_process_name,
        command_summary,
        work_area,
    }
}

fn window_summary(handle: RawWindowHandle) -> DebugWindowSummary {
    let hwnd = get_hwnd_from_raw(handle).map(|hwnd| hwnd.0 as isize);
    DebugWindowSummary {
        process_name: handle.get_app_process_name(),
        hwnd,
    }
}

fn raw_window(handle: RawWindowHandle, role: DebugWindowRole, tags: Vec<String>) -> DebugRawWindow {
    let summary = window_summary(handle);
    DebugRawWindow {
        role,
        process_name: summary.process_name,
        hwnd: summary.hwnd,
        tags,
    }
}

pub(crate) fn close_debug_overlay(ctx: &egui::Context) {
    ctx.send_viewport_cmd_to(
        egui::ViewportId::from_hash_of(DEBUG_OVERLAY_VIEWPORT_ID),
        egui::ViewportCommand::Close,
    );
}

pub(crate) fn show_debug_overlay(
    ctx: &egui::Context,
    snapshot: DebugSnapshot,
    palette_state: Option<DebugPaletteState>,
    event_tx: Sender<UiEvent>,
) {
    let (position, height) = debug_overlay_geometry(ctx, snapshot.work_area);
    ctx.show_viewport_deferred(
        egui::ViewportId::from_hash_of(DEBUG_OVERLAY_VIEWPORT_ID),
        egui::ViewportBuilder::default()
            .with_title("Omni Palette Debug")
            .with_inner_size([DEBUG_OVERLAY_WIDTH, height])
            .with_position(position)
            .with_decorations(true)
            .with_transparent(false)
            .with_always_on_top()
            .with_resizable(true)
            .with_active(false),
        move |ui, _class| {
            if ui.ctx().input(|input| input.viewport().close_requested()) {
                let _ = event_tx.send(UiEvent::DebuggerClosed);
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            draw_debug_overlay(ui, &snapshot, palette_state.as_ref());
        },
    );
}

fn debug_overlay_geometry(
    ctx: &egui::Context,
    work_area: Option<PaletteWorkArea>,
) -> (egui::Pos2, f32) {
    let native_pixels_per_point = ctx
        .input(|input| input.viewport().native_pixels_per_point)
        .unwrap_or(1.0)
        .max(1.0);

    if let Some(work_area) = work_area {
        let work_area = work_area.to_points(native_pixels_per_point);
        let height = DEBUG_OVERLAY_HEIGHT
            .min((work_area.height() - (DEBUG_OVERLAY_MARGIN * 2.0)).max(240.0));
        return (
            egui::pos2(
                work_area.right - DEBUG_OVERLAY_WIDTH - DEBUG_OVERLAY_MARGIN,
                work_area.top + DEBUG_OVERLAY_MARGIN,
            ),
            height,
        );
    }

    let monitor_size = ctx
        .input(|input| input.viewport().monitor_size)
        .unwrap_or(egui::vec2(1920.0, 1080.0));
    (
        egui::pos2(
            monitor_size.x - DEBUG_OVERLAY_WIDTH - DEBUG_OVERLAY_MARGIN,
            DEBUG_OVERLAY_MARGIN,
        ),
        DEBUG_OVERLAY_HEIGHT.min((monitor_size.y - (DEBUG_OVERLAY_MARGIN * 2.0)).max(240.0)),
    )
}

fn draw_debug_overlay(
    ui: &mut egui::Ui,
    snapshot: &DebugSnapshot,
    palette_state: Option<&DebugPaletteState>,
) {
    let text = egui::Color32::from_rgb(238, 242, 247);
    let muted = egui::Color32::from_rgb(171, 181, 196);
    let accent = egui::Color32::from_rgb(114, 230, 194);
    let warning = egui::Color32::from_rgb(255, 203, 120);

    egui::Frame::new()
        .fill(egui::Color32::from_rgba_premultiplied(14, 18, 24, 218))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_premultiplied(255, 255, 255, 34),
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(DEBUG_OVERLAY_WIDTH - 24.0);
            ui.label(
                egui::RichText::new("Debug Context")
                    .size(18.0)
                    .strong()
                    .color(text),
            );
            ui.label(
                egui::RichText::new("Process names, context tags, command ranking inputs")
                    .size(12.0)
                    .color(muted),
            );
            ui.add_space(12.0);

            section_label(ui, "Foreground", accent);
            window_line(ui, snapshot.foreground_window.as_ref(), text, muted);
            if let Some(process) = &snapshot.ignored_process_name {
                ui.label(
                    egui::RichText::new(format!("Passthrough ignored app: {process}"))
                        .size(12.0)
                        .color(warning),
                );
            }
            ui.add_space(10.0);

            section_label(ui, "Interaction", accent);
            ui.label(
                egui::RichText::new(format!("Text input: {}", snapshot.text_input_active))
                    .size(12.0)
                    .color(text),
            );
            let tags = if snapshot.active_tags.is_empty() {
                "none".to_string()
            } else {
                snapshot.active_tags.join(", ")
            };
            ui.label(
                egui::RichText::new(format!("Tags: {tags}"))
                    .size(12.0)
                    .color(muted),
            );
            ui.add_space(10.0);

            section_label(ui, "Command Candidates", accent);
            let summary = &snapshot.command_summary;
            ui.label(
                egui::RichText::new(format!(
                    "Total {} | Focused {} | Background {} | Global {}",
                    summary.total, summary.focused, summary.background, summary.global
                ))
                .size(12.0)
                .color(text),
            );
            ui.label(
                egui::RichText::new(format!(
                    "Favorites {} | Priority H:{} M:{} L:{} S:{}",
                    summary.favorites,
                    summary.high_priority,
                    summary.medium_priority,
                    summary.low_priority,
                    summary.suppressed_priority
                ))
                .size(12.0)
                .color(muted),
            );
            ui.add_space(10.0);

            if let Some(palette_state) = palette_state {
                section_label(ui, "Palette Filter", accent);
                ui.label(
                    egui::RichText::new(format!(
                        "Query {:?} | Results {}",
                        palette_state.query, palette_state.filtered_count
                    ))
                    .size(12.0)
                    .color(text),
                );
                for row in &palette_state.top_rows {
                    let tags = if row.tags.is_empty() {
                        "tags: none".to_string()
                    } else {
                        format!("tags: {}", row.tags.join(", "))
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "{} [{} {:?} {:?} score {} | {}]",
                            row.label,
                            if row.favorite { "fav" } else { "-" },
                            row.focus_state,
                            row.priority,
                            row.score,
                            tags
                        ))
                        .size(11.0)
                        .color(muted),
                    );
                }
                ui.add_space(10.0);
            }

            section_label(ui, "Background Windows", accent);
            ui.label(
                egui::RichText::new(format!(
                    "Showing {} of {}",
                    snapshot.background_windows.len(),
                    snapshot.background_total
                ))
                .size(12.0)
                .color(text),
            );
            for window in &snapshot.background_windows {
                window_line(ui, Some(window), text, muted);
            }
            ui.add_space(10.0);

            section_label(ui, "Raw Window Data", accent);
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for window in &snapshot.raw_windows {
                        raw_window_line(ui, window, muted);
                    }
                });
        });
}

fn section_label(ui: &mut egui::Ui, label: &str, color: egui::Color32) {
    ui.label(
        egui::RichText::new(label.to_ascii_uppercase())
            .size(11.0)
            .strong()
            .color(color),
    );
}

fn window_line(
    ui: &mut egui::Ui,
    window: Option<&DebugWindowSummary>,
    text: egui::Color32,
    muted: egui::Color32,
) {
    match window {
        Some(window) => {
            let process = window.process_name.as_deref().unwrap_or("unknown-process");
            let hwnd = window
                .hwnd
                .map(|hwnd| hwnd.to_string())
                .unwrap_or_else(|| "unknown-hwnd".to_string());
            ui.label(
                egui::RichText::new(format!("{process}  hwnd={hwnd}"))
                    .size(12.0)
                    .color(text),
            );
        }
        None => {
            ui.label(
                egui::RichText::new("No foreground window")
                    .size(12.0)
                    .color(muted),
            );
        }
    }
}

fn raw_window_line(ui: &mut egui::Ui, window: &DebugRawWindow, color: egui::Color32) {
    let process = window.process_name.as_deref().unwrap_or("unknown-process");
    let hwnd = window
        .hwnd
        .map(|hwnd| hwnd.to_string())
        .unwrap_or_else(|| "unknown-hwnd".to_string());
    let tags = if window.tags.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", window.tags.join(", "))
    };
    ui.label(
        egui::RichText::new(format!(
            "{} hwnd={} process={} tags={}",
            window.role.label(),
            hwnd,
            process,
            tags
        ))
        .size(11.0)
        .monospace()
        .color(color),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        DebugCommandCandidate, DebugCommandSummary, DebugSnapshot, DebugWindowRole,
        DebugWindowSummary, MAX_DEBUG_BACKGROUND_WINDOWS,
    };
    use crate::domain::action::{CommandPriority, FocusState, InteractionContext};

    #[test]
    fn command_summary_counts_candidates_by_focus_priority_and_favorites() {
        let summary = DebugCommandSummary::from_candidates([
            DebugCommandCandidate {
                focus_state: FocusState::Focused,
                priority: CommandPriority::High,
                favorite: true,
            },
            DebugCommandCandidate {
                focus_state: FocusState::Background,
                priority: CommandPriority::Low,
                favorite: false,
            },
            DebugCommandCandidate {
                focus_state: FocusState::Global,
                priority: CommandPriority::High,
                favorite: true,
            },
            DebugCommandCandidate {
                focus_state: FocusState::Global,
                priority: CommandPriority::Suppressed,
                favorite: false,
            },
        ]);

        assert_eq!(summary.total, 4);
        assert_eq!(summary.focused, 1);
        assert_eq!(summary.background, 1);
        assert_eq!(summary.global, 2);
        assert_eq!(summary.favorites, 2);
        assert_eq!(summary.high_priority, 2);
        assert_eq!(summary.low_priority, 1);
        assert_eq!(summary.suppressed_priority, 1);
    }

    #[test]
    fn snapshot_derives_text_input_from_active_interaction_tags() {
        let snapshot = DebugSnapshot::from_parts(
            None,
            Vec::new(),
            InteractionContext::from_tags(["ui.text_input".to_string()]),
            None,
            DebugCommandSummary::default(),
        );

        assert!(snapshot.text_input_active);
    }

    #[test]
    fn snapshot_caps_background_windows_but_keeps_total_count() {
        let background_windows = (0..15)
            .map(|index| DebugWindowSummary {
                process_name: Some(format!("app-{index}.exe")),
                hwnd: Some(index),
            })
            .collect();

        let snapshot = DebugSnapshot::from_parts(
            None,
            background_windows,
            InteractionContext::default(),
            None,
            DebugCommandSummary::default(),
        );

        assert_eq!(snapshot.background_total, 15);
        assert_eq!(
            snapshot.background_windows.len(),
            MAX_DEBUG_BACKGROUND_WINDOWS
        );
        assert_eq!(
            snapshot
                .background_windows
                .last()
                .and_then(|window| window.process_name.as_deref()),
            Some("app-11.exe")
        );
    }

    #[test]
    fn snapshot_includes_raw_window_data_with_active_tags_attached_to_foreground() {
        let snapshot = DebugSnapshot::from_parts(
            Some(DebugWindowSummary {
                process_name: Some("active.exe".to_string()),
                hwnd: Some(100),
            }),
            vec![DebugWindowSummary {
                process_name: Some("background.exe".to_string()),
                hwnd: Some(200),
            }],
            InteractionContext::from_tags(["ui.text_input".to_string(), "ui.edit".to_string()]),
            None,
            DebugCommandSummary::default(),
        );

        assert_eq!(snapshot.raw_windows.len(), 2);
        assert_eq!(snapshot.raw_windows[0].role, DebugWindowRole::Foreground);
        assert_eq!(
            snapshot.raw_windows[0].tags,
            vec!["ui.edit".to_string(), "ui.text_input".to_string()]
        );
        assert_eq!(snapshot.raw_windows[1].role, DebugWindowRole::Background);
        assert!(snapshot.raw_windows[1].tags.is_empty());
    }
}
