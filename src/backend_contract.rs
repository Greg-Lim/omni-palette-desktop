use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    core::{
        command_filter::{filter_commands, FilterableCommand},
        extensions::discovery::ExtensionDiscovery,
        registry::registry::{MasterRegistry, UnitAction},
        search::MatchRange,
    },
    domain::action::{ActionExecution, ActionMetadata, CommandPriority, FocusState, Os},
    platform::platform_interface::get_all_context,
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

    fn from_unit_action(action: UnitAction, original_order: usize) -> Self {
        Self::deferred(
            CommandId::new(format!("action-{original_order}")),
            format!("{}: {}", action.app_name, action.action_name),
            action.shortcut_text,
            action.focus_state,
            action.execution,
            action.metadata,
            original_order,
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
            .map(|row| {
                self.commands[row.command_index].to_dto(row.label_matches, row.score)
            })
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
    registry: Arc<RwLock<MasterRegistry>>,
    bundled_extensions_root: PathBuf,
    current_os: Os,
    session: RwLock<CommandSession>,
}

impl PaletteBackend {
    pub fn default_for_bundled_root(root: impl AsRef<Path>, current_os: Os) -> Self {
        let bundled_extensions_root = root.as_ref().to_path_buf();
        let discovery = ExtensionDiscovery::bundled_with_user_root(&bundled_extensions_root);
        let registry = MasterRegistry::build(&discovery, current_os);

        Self {
            registry: Arc::new(RwLock::new(registry)),
            bundled_extensions_root,
            current_os,
            session: RwLock::new(CommandSession::from_commands(Vec::new())),
        }
    }

    pub fn get_palette_bootstrap(&self) -> PaletteBootstrapDto {
        let snapshot = self.search_commands("");

        PaletteBootstrapDto {
            session_id: snapshot.session_id,
            backend_status: "ok".to_string(),
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
            Err(err) => CommandExecutionResultDto::failed(format!(
                "Command session lock poisoned: {err}"
            )),
        }
    }

    fn build_current_session(&self) -> Result<CommandSession, String> {
        let context = get_all_context();
        let registry = self
            .registry
            .read()
            .map_err(|err| format!("Extension registry lock poisoned: {err}"))?;
        let mut commands = vec![self.reload_extensions_command(0)];

        commands.extend(
            registry
                .get_actions(&context)
                .into_iter()
                .enumerate()
                .map(|(index, action)| BackendCommand::from_unit_action(action, index + 1)),
        );

        Ok(CommandSession::from_commands(commands))
    }

    fn reload_extensions_command(&self, original_order: usize) -> BackendCommand {
        let registry = Arc::clone(&self.registry);
        let bundled_extensions_root = self.bundled_extensions_root.clone();
        let current_os = self.current_os;

        BackendCommand::reload_extensions(
            CommandId::new("reload-extensions"),
            original_order,
            Box::new(move || {
                reload_registry(&registry, &bundled_extensions_root, current_os).map(|count| {
                    format!("Reloaded extensions: {count} applications")
                })
            }),
        )
    }
}

fn reload_registry(
    registry: &Arc<RwLock<MasterRegistry>>,
    bundled_extensions_root: &Path,
    current_os: Os,
) -> Result<usize, String> {
    let discovery = ExtensionDiscovery::bundled_with_user_root(bundled_extensions_root);
    let new_registry =
        MasterRegistry::build_strict(&discovery, current_os).map_err(|err| err.to_string())?;
    let application_count = new_registry.application_registry.len();
    let mut registry = registry
        .write()
        .map_err(|err| format!("Extension registry lock poisoned: {err}"))?;
    *registry = new_registry;
    Ok(application_count)
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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use crate::{
        core::search::MatchRange,
        domain::{
            action::{ActionExecution, ActionMetadata, CommandPriority, FocusState},
            hotkey::{HotkeyModifiers, Key, KeyboardShortcut},
        },
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
        assert_eq!(labels, vec!["Chrome: Close tab", "Windows: Open File Explorer"]);
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
            CommandExecutionResultDto::succeeded("Reloaded extensions: 1 applications, 0 ignored processes")
        );
    }
}
