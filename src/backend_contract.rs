use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::HWND;

use crate::{
    core::{
        command_filter::{filter_commands, FilterableCommand},
        plugins::PluginRegistry,
        registry::registry::UnitAction,
        search::MatchRange,
    },
    domain::{
        action::{
            ActionExecution, ActionMetadata, CommandPriority, FocusState, InteractionContext,
            KeySequenceStep, Os,
        },
        hotkey::KeyboardShortcut,
    },
    platform::{
        platform_interface::get_all_context,
        windows::{
            context::context::{focus_window, get_hwnd_from_raw},
            sender::hotkey_sender::{send_shortcut, send_shortcut_sequence},
        },
    },
    runtime_state::{OmniRuntimeState, ReloadReport, RuntimeStateLoadOptions, RuntimeStatusDto},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommandId(String);

impl CommandId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaletteSessionId(String);

impl PaletteSessionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchRangeDto {
    pub start: usize,
    pub end: usize,
}

impl From<MatchRange> for MatchRangeDto {
    fn from(range: MatchRange) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandDto {
    pub id: CommandId,
    pub label: String,
    pub shortcut_text: String,
    pub focus_state: FocusState,
    pub priority: CommandPriority,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub original_order: usize,
    pub score: i32,
    pub label_matches: Vec<MatchRangeDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteSnapshotDto {
    pub session_id: PaletteSessionId,
    pub query: String,
    pub commands: Vec<CommandDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteBootstrapDto {
    pub session_id: PaletteSessionId,
    pub backend_status: String,
    pub runtime_status: RuntimeStatusDto,
    pub commands: Vec<CommandDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandExecutionStatus {
    Succeeded,
    Failed,
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandExecutionResultDto {
    pub status: CommandExecutionStatus,
    pub message: String,
}

impl CommandExecutionResultDto {
    pub fn succeeded(message: impl Into<String>) -> Self {
        Self {
            status: CommandExecutionStatus::Succeeded,
            message: message.into(),
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            status: CommandExecutionStatus::Failed,
            message: message.into(),
        }
    }

    pub fn deferred(message: impl Into<String>) -> Self {
        Self {
            status: CommandExecutionStatus::Deferred,
            message: message.into(),
        }
    }
}

type ReloadExtensionsFn = dyn Fn() -> Result<String, String> + Send + Sync;
type SharedCommandExecutor = Arc<dyn CommandExecutor>;

trait CommandExecutor: Send + Sync {
    fn execute_shortcut(
        &self,
        shortcut: KeyboardShortcut,
        focus_target: Option<isize>,
    ) -> Result<(), String>;

    fn execute_shortcut_sequence(
        &self,
        sequence: &[KeySequenceStep],
        focus_target: Option<isize>,
    ) -> Result<(), String>;

    fn execute_plugin_command(
        &self,
        plugin_registry: Arc<PluginRegistry>,
        plugin_id: &str,
        command_id: &str,
        active_hwnd_val: Option<isize>,
        active_interaction: InteractionContext,
    ) -> Result<(), String>;
}

struct WindowsCommandExecutor;

impl CommandExecutor for WindowsCommandExecutor {
    fn execute_shortcut(
        &self,
        shortcut: KeyboardShortcut,
        focus_target: Option<isize>,
    ) -> Result<(), String> {
        if let Some(val) = focus_target {
            focus_window_from_value(val);
        }
        send_shortcut(&shortcut);
        Ok(())
    }

    fn execute_shortcut_sequence(
        &self,
        sequence: &[KeySequenceStep],
        focus_target: Option<isize>,
    ) -> Result<(), String> {
        if let Some(val) = focus_target {
            focus_window_from_value(val);
        }
        send_shortcut_sequence(sequence);
        Ok(())
    }

    fn execute_plugin_command(
        &self,
        plugin_registry: Arc<PluginRegistry>,
        plugin_id: &str,
        command_id: &str,
        active_hwnd_val: Option<isize>,
        active_interaction: InteractionContext,
    ) -> Result<(), String> {
        if let Some(val) = active_hwnd_val {
            focus_window_from_value(val);
            std::thread::sleep(Duration::from_millis(75));
        }
        plugin_registry.execute_with_context(plugin_id, command_id, active_interaction)
    }
}

fn focus_window_from_value(hwnd_val: isize) {
    focus_window(HWND(hwnd_val as *mut _));
}

struct RuntimeActionContext {
    shortcut_focus_target: Option<isize>,
    active_hwnd_val: Option<isize>,
    active_interaction: InteractionContext,
    plugin_registry: Arc<PluginRegistry>,
}

struct RuntimeActionCommand {
    execution: ActionExecution,
    context: RuntimeActionContext,
    executor: SharedCommandExecutor,
}

impl RuntimeActionCommand {
    fn execute(&self) -> Result<(), String> {
        match &self.execution {
            ActionExecution::Shortcut(shortcut) => self
                .executor
                .execute_shortcut(*shortcut, self.context.shortcut_focus_target),
            ActionExecution::ShortcutSequence(sequence) => self
                .executor
                .execute_shortcut_sequence(sequence, self.context.shortcut_focus_target),
            ActionExecution::PluginCommand {
                plugin_id,
                command_id,
            } => self.executor.execute_plugin_command(
                Arc::clone(&self.context.plugin_registry),
                plugin_id,
                command_id,
                self.context.active_hwnd_val,
                self.context.active_interaction.clone(),
            ),
        }
    }
}

pub struct BackendCommand {
    id: CommandId,
    label: String,
    shortcut_text: String,
    focus_state: FocusState,
    execution: StoredExecution,
    metadata: ActionMetadata,
    original_order: usize,
}

enum StoredExecution {
    Deferred(ActionExecution),
    RuntimeAction(RuntimeActionCommand),
    ReloadExtensions(Box<ReloadExtensionsFn>),
}

impl BackendCommand {
    pub fn deferred(
        id: CommandId,
        label: String,
        shortcut_text: String,
        focus_state: FocusState,
        execution: ActionExecution,
        metadata: ActionMetadata,
        original_order: usize,
    ) -> Self {
        Self {
            id,
            label,
            shortcut_text,
            focus_state,
            execution: StoredExecution::Deferred(execution),
            metadata,
            original_order,
        }
    }

    fn runtime_action(
        id: CommandId,
        label: String,
        shortcut_text: String,
        focus_state: FocusState,
        execution: ActionExecution,
        metadata: ActionMetadata,
        original_order: usize,
        context: RuntimeActionContext,
        executor: SharedCommandExecutor,
    ) -> Self {
        Self {
            id,
            label,
            shortcut_text,
            focus_state,
            execution: StoredExecution::RuntimeAction(RuntimeActionCommand {
                execution,
                context,
                executor,
            }),
            metadata,
            original_order,
        }
    }

    pub fn reload_extensions(
        id: CommandId,
        original_order: usize,
        reload: Box<ReloadExtensionsFn>,
    ) -> Self {
        Self {
            id,
            label: "Omni Palette: Reload extensions".to_string(),
            shortcut_text: String::new(),
            focus_state: FocusState::Global,
            execution: StoredExecution::ReloadExtensions(reload),
            metadata: ActionMetadata {
                priority: CommandPriority::Medium,
                favorite: false,
                tags: vec!["extensions".to_string(), "reload".to_string()],
            },
            original_order,
        }
    }

    fn from_unit_action(
        action: UnitAction,
        original_order: usize,
        plugin_registry: Arc<PluginRegistry>,
        active_hwnd_val: Option<isize>,
        active_interaction: InteractionContext,
        executor: SharedCommandExecutor,
    ) -> Self {
        let target_hwnd_val = action
            .target_window
            .and_then(get_hwnd_from_raw)
            .map(|hwnd| hwnd.0 as isize);
        let shortcut_focus_target = target_hwnd_val.or(active_hwnd_val);

        Self::runtime_action(
            CommandId::new(format!("action-{original_order}")),
            format!("{}: {}", action.app_name, action.action_name),
            action.shortcut_text,
            action.focus_state,
            action.execution,
            action.metadata,
            original_order,
            RuntimeActionContext {
                shortcut_focus_target,
                active_hwnd_val,
                active_interaction,
                plugin_registry,
            },
            executor,
        )
    }

    pub fn to_dto(&self, label_matches: Vec<MatchRange>, score: i32) -> CommandDto {
        CommandDto {
            id: self.id.clone(),
            label: self.label.clone(),
            shortcut_text: self.shortcut_text.clone(),
            focus_state: self.focus_state,
            priority: self.metadata.priority,
            favorite: self.metadata.favorite,
            tags: self.metadata.tags.clone(),
            original_order: self.original_order,
            score,
            label_matches: label_matches.into_iter().map(Into::into).collect(),
        }
    }
}

impl FilterableCommand for BackendCommand {
    fn label(&self) -> &str {
        &self.label
    }

    fn priority(&self) -> CommandPriority {
        self.metadata.priority
    }

    fn focus_state(&self) -> FocusState {
        self.focus_state
    }

    fn favorite(&self) -> bool {
        self.metadata.favorite
    }

    fn tags(&self) -> &[String] {
        &self.metadata.tags
    }

    fn original_order(&self) -> usize {
        self.original_order
    }
}

pub struct CommandSession {
    session_id: PaletteSessionId,
    commands: Vec<BackendCommand>,
    command_index: HashMap<String, usize>,
}

impl CommandSession {
    pub fn from_commands(commands: Vec<BackendCommand>) -> Self {
        let command_index = commands
            .iter()
            .enumerate()
            .map(|(index, command)| (command.id.value().to_string(), index))
            .collect();

        Self {
            session_id: new_session_id(),
            commands,
            command_index,
        }
    }

    pub fn search(&self, query: &str) -> PaletteSnapshotDto {
        let commands = filter_commands(&self.commands, query)
            .into_iter()
            .map(|row| self.commands[row.command_index].to_dto(row.label_matches, row.score))
            .collect();

        PaletteSnapshotDto {
            session_id: self.session_id.clone(),
            query: query.to_string(),
            commands,
        }
    }

    pub fn execute(&self, command_id: &CommandId) -> CommandExecutionResultDto {
        let Some(command) = self
            .command_index
            .get(command_id.value())
            .and_then(|index| self.commands.get(*index))
        else {
            return CommandExecutionResultDto::failed(format!(
                "Unknown or stale command id: {}",
                command_id.value()
            ));
        };

        match &command.execution {
            StoredExecution::ReloadExtensions(reload) => match reload() {
                Ok(message) => CommandExecutionResultDto::succeeded(message),
                Err(err) => CommandExecutionResultDto::failed(err),
            },
            StoredExecution::RuntimeAction(action) => match action.execute() {
                Ok(()) => {
                    CommandExecutionResultDto::succeeded(format!("Executed {}", command.label))
                }
                Err(err) => CommandExecutionResultDto::failed(format!(
                    "Failed to execute {}: {err}",
                    command.label
                )),
            },
            StoredExecution::Deferred(execution) => {
                let execution_kind = match execution {
                    ActionExecution::Shortcut(_) => "shortcut",
                    ActionExecution::ShortcutSequence(_) => "shortcut sequence",
                    ActionExecution::PluginCommand { .. } => "plugin command",
                };

                CommandExecutionResultDto::deferred(format!(
                    "{} ({execution_kind}) is deferred until runtime integration",
                    command.label
                ))
            }
        }
    }
}

pub struct PaletteBackend {
    runtime_state: OmniRuntimeState,
    command_executor: SharedCommandExecutor,
    session: RwLock<CommandSession>,
}

impl PaletteBackend {
    pub fn default_for_bundled_root(root: impl AsRef<Path>, current_os: Os) -> Self {
        Self::from_runtime_state(OmniRuntimeState::load(
            RuntimeStateLoadOptions::from_environment(root, current_os),
        ))
    }

    pub fn from_runtime_state(runtime_state: OmniRuntimeState) -> Self {
        Self::with_command_executor(runtime_state, Arc::new(WindowsCommandExecutor))
    }

    fn with_command_executor(
        runtime_state: OmniRuntimeState,
        command_executor: SharedCommandExecutor,
    ) -> Self {
        Self {
            runtime_state,
            command_executor,
            session: RwLock::new(CommandSession::from_commands(Vec::new())),
        }
    }

    pub fn get_palette_bootstrap(&self) -> PaletteBootstrapDto {
        let snapshot = self.search_commands("");

        PaletteBootstrapDto {
            session_id: snapshot.session_id,
            backend_status: "ok".to_string(),
            runtime_status: self.runtime_state.status(),
            commands: snapshot.commands,
        }
    }

    pub fn search_commands(&self, query: &str) -> PaletteSnapshotDto {
        match self.build_current_session() {
            Ok(session) => {
                let snapshot = session.search(query);
                if let Ok(mut current_session) = self.session.write() {
                    *current_session = session;
                }
                snapshot
            }
            Err(err) => PaletteSnapshotDto {
                session_id: new_session_id(),
                query: query.to_string(),
                commands: vec![backend_error_command(err).to_dto(Vec::new(), 0)],
            },
        }
    }

    pub fn execute_command(&self, command_id: &CommandId) -> CommandExecutionResultDto {
        match self.session.read() {
            Ok(session) => session.execute(command_id),
            Err(err) => {
                CommandExecutionResultDto::failed(format!("Command session lock poisoned: {err}"))
            }
        }
    }

    fn build_current_session(&self) -> Result<CommandSession, String> {
        let context = get_all_context();
        let active_hwnd_val = context
            .get_active()
            .and_then(|handle| get_hwnd_from_raw(*handle))
            .map(|hwnd| hwnd.0 as isize);
        let active_interaction = context.active_interaction.clone();
        let registry = self.runtime_state.registry();
        let registry = registry
            .read()
            .map_err(|err| format!("Extension registry lock poisoned: {err}"))?;
        let plugin_registry = registry.plugin_registry();
        let command_executor = Arc::clone(&self.command_executor);
        let mut commands = vec![self.reload_extensions_command(0)];

        commands.extend(registry.get_actions(&context).into_iter().enumerate().map(
            |(index, action)| {
                BackendCommand::from_unit_action(
                    action,
                    index + 1,
                    Arc::clone(&plugin_registry),
                    active_hwnd_val,
                    active_interaction.clone(),
                    Arc::clone(&command_executor),
                )
            },
        ));

        Ok(CommandSession::from_commands(commands))
    }

    fn reload_extensions_command(&self, original_order: usize) -> BackendCommand {
        let runtime_state = self.runtime_state.clone();

        BackendCommand::reload_extensions(
            CommandId::new("reload-extensions"),
            original_order,
            Box::new(move || runtime_state.reload_extensions().map(reload_report_message)),
        )
    }
}

fn reload_report_message(report: ReloadReport) -> String {
    format!(
        "Reloaded extensions: {} applications, {} ignored processes",
        report.application_count, report.ignored_process_count
    )
}

fn backend_error_command(message: String) -> BackendCommand {
    BackendCommand::deferred(
        CommandId::new("backend-error"),
        format!("Backend unavailable: {message}"),
        String::new(),
        FocusState::Global,
        ActionExecution::PluginCommand {
            plugin_id: "omni_palette".to_string(),
            command_id: "backend-error".to_string(),
        },
        ActionMetadata {
            priority: CommandPriority::High,
            favorite: false,
            tags: vec!["backend".to_string(), "error".to_string()],
        },
        0,
    )
}

fn new_session_id() -> PaletteSessionId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    PaletteSessionId::new(format!("session-{nanos}"))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };

    use crate::{
        config::runtime::RuntimePaths,
        core::plugins::PluginRegistry,
        core::search::MatchRange,
        domain::{
            action::{
                ActionExecution, ActionMetadata, CommandPriority, FocusState, InteractionContext,
                KeySequenceStep, SequenceKey,
            },
            hotkey::{HotkeyModifiers, Key, KeyboardShortcut},
        },
        runtime_state::{OmniRuntimeState, RuntimeStateLoadOptions},
    };

    use super::*;

    fn shortcut_execution() -> ActionExecution {
        ActionExecution::Shortcut(KeyboardShortcut {
            modifier: HotkeyModifiers {
                control: true,
                ..Default::default()
            },
            key: Key::KeyT,
        })
    }

    fn metadata(priority: CommandPriority, favorite: bool, tags: &[&str]) -> ActionMetadata {
        ActionMetadata {
            priority,
            favorite,
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
        }
    }

    fn test_command(
        id: &str,
        label: &str,
        shortcut_text: &str,
        priority: CommandPriority,
        focus_state: FocusState,
        favorite: bool,
        tags: &[&str],
        original_order: usize,
    ) -> BackendCommand {
        BackendCommand::deferred(
            CommandId::new(id),
            label.to_string(),
            shortcut_text.to_string(),
            focus_state,
            shortcut_execution(),
            metadata(priority, favorite, tags),
            original_order,
        )
    }

    fn runtime_action_command(
        id: &str,
        label: &str,
        execution: ActionExecution,
        executor: Arc<dyn CommandExecutor>,
    ) -> BackendCommand {
        BackendCommand::runtime_action(
            CommandId::new(id),
            label.to_string(),
            "Ctrl+T".to_string(),
            FocusState::Focused,
            execution,
            metadata(CommandPriority::Medium, false, &["runtime"]),
            0,
            RuntimeActionContext {
                shortcut_focus_target: Some(44),
                active_hwnd_val: Some(55),
                active_interaction: InteractionContext::from_tags(["selection.url".to_string()]),
                plugin_registry: Arc::new(PluginRegistry::default()),
            },
            executor,
        )
    }

    #[test]
    fn command_dto_preserves_runtime_fields_and_match_ranges() {
        let command = test_command(
            "cmd-1",
            "Chrome: New tab",
            "Ctrl+T",
            CommandPriority::High,
            FocusState::Focused,
            true,
            &["browser", "tabs"],
            7,
        );

        let dto = command.to_dto(vec![MatchRange { start: 8, end: 11 }], 42);

        assert_eq!(dto.id.value(), "cmd-1");
        assert_eq!(dto.label, "Chrome: New tab");
        assert_eq!(dto.shortcut_text, "Ctrl+T");
        assert_eq!(dto.priority, CommandPriority::High);
        assert_eq!(dto.focus_state, FocusState::Focused);
        assert!(dto.favorite);
        assert_eq!(dto.tags, vec!["browser".to_string(), "tabs".to_string()]);
        assert_eq!(dto.original_order, 7);
        assert_eq!(dto.score, 42);
        assert_eq!(dto.label_matches, vec![MatchRangeDto { start: 8, end: 11 }]);
    }

    #[test]
    fn empty_query_uses_existing_priority_and_focus_sorting() {
        let session = CommandSession::from_commands(vec![
            test_command(
                "global-high",
                "Windows: Open File Explorer",
                "",
                CommandPriority::High,
                FocusState::Global,
                false,
                &[],
                0,
            ),
            test_command(
                "focused-low",
                "Chrome: Close tab",
                "Ctrl+W",
                CommandPriority::Low,
                FocusState::Focused,
                false,
                &[],
                1,
            ),
        ]);

        let snapshot = session.search("");

        let labels = snapshot
            .commands
            .iter()
            .map(|command| command.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec!["Chrome: Close tab", "Windows: Open File Explorer"]
        );
    }

    #[test]
    fn non_empty_query_returns_fuzzy_match_ranges() {
        let session = CommandSession::from_commands(vec![test_command(
            "cmd-1",
            "Chrome: New tab",
            "Ctrl+T",
            CommandPriority::Medium,
            FocusState::Focused,
            false,
            &["browser"],
            0,
        )]);

        let snapshot = session.search("new");

        assert_eq!(snapshot.commands.len(), 1);
        assert_eq!(snapshot.commands[0].label, "Chrome: New tab");
        assert!(!snapshot.commands[0].label_matches.is_empty());
    }

    #[test]
    fn unknown_command_id_returns_controlled_result() {
        let session = CommandSession::from_commands(Vec::new());

        let result = session.execute(&CommandId::new("missing"));

        assert_eq!(
            result,
            CommandExecutionResultDto::failed("Unknown or stale command id: missing")
        );
    }

    #[test]
    fn built_in_reload_command_dispatches_callback() {
        let called = Arc::new(AtomicBool::new(false));
        let called_by_command = Arc::clone(&called);
        let session = CommandSession::from_commands(vec![BackendCommand::reload_extensions(
            CommandId::new("reload"),
            0,
            Box::new(move || {
                called_by_command.store(true, Ordering::Relaxed);
                Ok("Reloaded extensions: 1 applications, 0 ignored processes".to_string())
            }),
        )]);

        let result = session.execute(&CommandId::new("reload"));

        assert!(called.load(Ordering::Relaxed));
        assert_eq!(
            result,
            CommandExecutionResultDto::succeeded(
                "Reloaded extensions: 1 applications, 0 ignored processes"
            )
        );
    }

    #[test]
    fn shortcut_command_dispatches_through_executor() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let executor = Arc::new(RecordingCommandExecutor::new(Arc::clone(&calls), Ok(())));
        let shortcut = KeyboardShortcut {
            modifier: HotkeyModifiers {
                control: true,
                ..Default::default()
            },
            key: Key::KeyT,
        };
        let session = CommandSession::from_commands(vec![runtime_action_command(
            "chrome-new-tab",
            "Chrome: New tab",
            ActionExecution::Shortcut(shortcut),
            executor,
        )]);

        let result = session.execute(&CommandId::new("chrome-new-tab"));

        assert_eq!(
            result,
            CommandExecutionResultDto::succeeded("Executed Chrome: New tab")
        );
        assert_eq!(
            recorded_calls(&calls),
            vec![RecordedExecution::Shortcut {
                shortcut,
                focus_target: Some(44),
            }]
        );
    }

    #[test]
    fn shortcut_sequence_command_dispatches_through_executor() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let executor = Arc::new(RecordingCommandExecutor::new(Arc::clone(&calls), Ok(())));
        let sequence = vec![KeySequenceStep {
            modifier: HotkeyModifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
            key: SequenceKey::Key(Key::KeyK),
        }];
        let session = CommandSession::from_commands(vec![runtime_action_command(
            "vscode-command-palette",
            "VS Code: Command Palette",
            ActionExecution::ShortcutSequence(sequence.clone()),
            executor,
        )]);

        let result = session.execute(&CommandId::new("vscode-command-palette"));

        assert_eq!(
            result,
            CommandExecutionResultDto::succeeded("Executed VS Code: Command Palette")
        );
        assert_eq!(
            recorded_calls(&calls),
            vec![RecordedExecution::ShortcutSequence {
                sequence,
                focus_target: Some(44),
            }]
        );
    }

    #[test]
    fn plugin_command_dispatches_with_active_interaction_context() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let executor = Arc::new(RecordingCommandExecutor::new(Arc::clone(&calls), Ok(())));
        let session = CommandSession::from_commands(vec![runtime_action_command(
            "plugin-command",
            "Context Reader: Read URL",
            ActionExecution::PluginCommand {
                plugin_id: "context_reader".to_string(),
                command_id: "read_url".to_string(),
            },
            executor,
        )]);

        let result = session.execute(&CommandId::new("plugin-command"));

        assert_eq!(
            result,
            CommandExecutionResultDto::succeeded("Executed Context Reader: Read URL")
        );
        assert_eq!(
            recorded_calls(&calls),
            vec![RecordedExecution::Plugin {
                plugin_id: "context_reader".to_string(),
                command_id: "read_url".to_string(),
                active_hwnd_val: Some(55),
                active_interaction: InteractionContext::from_tags(["selection.url".to_string()]),
            }]
        );
    }

    #[test]
    fn plugin_command_failure_returns_controlled_failure() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let executor = Arc::new(RecordingCommandExecutor::new(
            Arc::clone(&calls),
            Err("plugin exploded".to_string()),
        ));
        let session = CommandSession::from_commands(vec![runtime_action_command(
            "plugin-command",
            "Context Reader: Read URL",
            ActionExecution::PluginCommand {
                plugin_id: "context_reader".to_string(),
                command_id: "read_url".to_string(),
            },
            executor,
        )]);

        let result = session.execute(&CommandId::new("plugin-command"));

        assert_eq!(
            result,
            CommandExecutionResultDto::failed(
                "Failed to execute Context Reader: Read URL: plugin exploded"
            )
        );
    }

    #[test]
    fn runtime_state_loads_config_registry_and_ignored_processes() {
        let root = runtime_test_root("loads-config-registry-and-ignore");
        write_static_extension(&root, "chrome", "Chrome", "chrome.exe");
        std::fs::write(root.join("ignore.toml"), "windows = [\"Code.exe\"]")
            .expect("ignore config should be written");

        let runtime = OmniRuntimeState::load(RuntimeStateLoadOptions {
            bundled_extensions_root: root.clone(),
            user_extensions_root: None,
            dev_config_path: root.join("config.toml"),
            runtime_paths: RuntimePaths {
                config_path: None,
                local_cache_root: None,
            },
            current_os: Os::Windows,
        });

        let status = runtime.status();

        assert_eq!(status.application_count, 1);
        assert_eq!(status.ignored_process_count, 1);
        assert_eq!(status.plugin_count, 0);
        assert_eq!(status.plugin_application_count, 0);
        assert_eq!(status.config_path, None);
        assert_eq!(status.config_error, None);
    }

    #[test]
    fn palette_bootstrap_reports_runtime_status() {
        let root = runtime_test_root("palette-bootstrap-runtime-status");
        write_static_extension(&root, "chrome", "Chrome", "chrome.exe");
        std::fs::write(root.join("ignore.toml"), "windows = [\"Code.exe\"]")
            .expect("ignore config should be written");

        let runtime = OmniRuntimeState::load(RuntimeStateLoadOptions {
            bundled_extensions_root: root,
            user_extensions_root: None,
            dev_config_path: PathBuf::from("missing-dev-config.toml"),
            runtime_paths: RuntimePaths {
                config_path: Some(PathBuf::from(
                    "C:/Users/example/AppData/Roaming/OmniPalette/config.toml",
                )),
                local_cache_root: None,
            },
            current_os: Os::Windows,
        });
        let backend = PaletteBackend::from_runtime_state(runtime);

        let bootstrap = backend.get_palette_bootstrap();

        assert_eq!(bootstrap.backend_status, "ok");
        assert_eq!(bootstrap.runtime_status.application_count, 1);
        assert_eq!(bootstrap.runtime_status.ignored_process_count, 1);
        assert_eq!(
            bootstrap.runtime_status.config_path.as_deref(),
            Some("C:/Users/example/AppData/Roaming/OmniPalette/config.toml")
        );
        assert_eq!(bootstrap.runtime_status.activation_hint, "Ctrl+Shift+P");
        assert!(!bootstrap.commands.is_empty());
    }

    #[test]
    fn reload_refreshes_registry_and_ignored_process_count() {
        let root = runtime_test_root("reload-refreshes-state");
        write_static_extension(&root, "chrome", "Chrome", "chrome.exe");
        std::fs::write(root.join("ignore.toml"), "windows = [\"Code.exe\"]")
            .expect("ignore config should be written");

        let runtime = OmniRuntimeState::load(RuntimeStateLoadOptions {
            bundled_extensions_root: root.clone(),
            user_extensions_root: None,
            dev_config_path: PathBuf::from("missing-dev-config.toml"),
            runtime_paths: RuntimePaths {
                config_path: None,
                local_cache_root: None,
            },
            current_os: Os::Windows,
        });

        write_static_extension(&root, "notepad", "Notepad", "notepad.exe");
        std::fs::write(
            root.join("ignore.toml"),
            "windows = [\"Code.exe\", \"notepad.exe\"]",
        )
        .expect("ignore config should be updated");

        let report = runtime.reload_extensions().expect("reload should succeed");

        assert_eq!(report.application_count, 2);
        assert_eq!(report.ignored_process_count, 2);
        assert_eq!(runtime.status().application_count, 2);
        assert_eq!(runtime.status().ignored_process_count, 2);
    }

    fn runtime_test_root(name: &str) -> PathBuf {
        let root = PathBuf::from("target")
            .join("runtime-state-tests")
            .join(name);
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("runtime test root should reset");
        }
        std::fs::create_dir_all(root.join("static")).expect("static dir should be created");
        root
    }

    fn write_static_extension(root: &Path, id: &str, name: &str, process_name: &str) {
        let content = format!(
            r#"
version = 2
platform = "windows"

[app]
id = "{id}"
name = "{name}"
process_name = "{process_name}"

[actions]

[actions.open]
name = "Open"
cmd = {{ mods = ["ctrl"], key = "KeyO" }}
"#
        );
        std::fs::write(root.join("static").join(format!("{id}.toml")), content)
            .expect("static extension should be written");
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum RecordedExecution {
        Shortcut {
            shortcut: KeyboardShortcut,
            focus_target: Option<isize>,
        },
        ShortcutSequence {
            sequence: Vec<KeySequenceStep>,
            focus_target: Option<isize>,
        },
        Plugin {
            plugin_id: String,
            command_id: String,
            active_hwnd_val: Option<isize>,
            active_interaction: InteractionContext,
        },
    }

    struct RecordingCommandExecutor {
        calls: Arc<Mutex<Vec<RecordedExecution>>>,
        result: Result<(), String>,
    }

    impl RecordingCommandExecutor {
        fn new(calls: Arc<Mutex<Vec<RecordedExecution>>>, result: Result<(), String>) -> Self {
            Self { calls, result }
        }
    }

    impl CommandExecutor for RecordingCommandExecutor {
        fn execute_shortcut(
            &self,
            shortcut: KeyboardShortcut,
            focus_target: Option<isize>,
        ) -> Result<(), String> {
            self.calls
                .lock()
                .expect("calls should lock")
                .push(RecordedExecution::Shortcut {
                    shortcut,
                    focus_target,
                });
            self.result.clone()
        }

        fn execute_shortcut_sequence(
            &self,
            sequence: &[KeySequenceStep],
            focus_target: Option<isize>,
        ) -> Result<(), String> {
            self.calls.lock().expect("calls should lock").push(
                RecordedExecution::ShortcutSequence {
                    sequence: sequence.to_vec(),
                    focus_target,
                },
            );
            self.result.clone()
        }

        fn execute_plugin_command(
            &self,
            _plugin_registry: Arc<PluginRegistry>,
            plugin_id: &str,
            command_id: &str,
            active_hwnd_val: Option<isize>,
            active_interaction: InteractionContext,
        ) -> Result<(), String> {
            self.calls
                .lock()
                .expect("calls should lock")
                .push(RecordedExecution::Plugin {
                    plugin_id: plugin_id.to_string(),
                    command_id: command_id.to_string(),
                    active_hwnd_val,
                    active_interaction,
                });
            self.result.clone()
        }
    }

    fn recorded_calls(calls: &Arc<Mutex<Vec<RecordedExecution>>>) -> Vec<RecordedExecution> {
        calls.lock().expect("calls should lock").clone()
    }
}
