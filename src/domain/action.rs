use crate::domain::hotkey::{HotkeyModifiers, Key, KeyboardShortcut};
use raw_window_handle::RawWindowHandle;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Hash)]
pub struct Action {
    pub name: String,
    pub execution: ActionExecution,
    pub shortcut_text: String,
    pub focus_state: FocusState,
    pub when: ActionContextCondition,
    pub metadata: ActionMetadata,
}

#[derive(Debug, Clone, Hash)]
pub enum ActionExecution {
    Shortcut(KeyboardShortcut),
    ShortcutSequence(Vec<KeySequenceStep>),
    PluginCommand {
        plugin_id: String,
        command_id: String,
    },
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct KeySequenceStep {
    pub modifier: HotkeyModifiers,
    pub key: SequenceKey,
}

impl KeySequenceStep {
    pub fn display_name(&self) -> String {
        let mut parts = Vec::new();
        if self.modifier.control {
            parts.push("Ctrl".to_string());
        }
        if self.modifier.shift {
            parts.push("Shift".to_string());
        }
        if self.modifier.alt {
            parts.push("Alt".to_string());
        }
        if self.modifier.win {
            parts.push("Win".to_string());
        }
        parts.push(self.key.display_name().to_string());
        parts.join("+")
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum SequenceKey {
    Key(Key),
    Ctrl,
    Shift,
    Alt,
}

impl SequenceKey {
    pub fn display_name(&self) -> &'static str {
        match self {
            SequenceKey::Key(key) => key.display_name(),
            SequenceKey::Ctrl => "Ctrl",
            SequenceKey::Shift => "Shift",
            SequenceKey::Alt => "Alt",
        }
    }
}

pub fn sequence_shortcut_text(steps: &[KeySequenceStep]) -> String {
    steps
        .iter()
        .map(KeySequenceStep::display_name)
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Clone, Hash)]
pub struct ActionMetadata {
    pub priority: CommandPriority,
    pub favorite: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Hash)]
pub struct ActionContextCondition {
    pub any: Vec<String>,
}

impl ActionContextCondition {
    pub fn matches(&self, context: &InteractionContext) -> bool {
        self.any.is_empty() || self.any.iter().any(|tag| context.has_tag(tag))
    }
}

#[derive(Debug, Deserialize, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum CommandPriority {
    #[serde(alias = "Suppressed")]
    Suppressed,
    #[serde(alias = "Low")]
    Low,
    #[default]
    #[serde(alias = "normal", alias = "Normal", alias = "Medium")]
    Medium,
    #[serde(alias = "High")]
    High,
}

#[derive(Debug, Deserialize, Copy, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum FocusState {
    Focused,
    Background,
    Global,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    #[serde(rename = "macos")]
    Mac,
    Linux,
}

pub type ApplicationID = u32;
pub type AppName = String;
pub type AppProcessName = String;
pub type ActionId = u32;
pub type ActionName = String;

#[derive(Debug, Clone, Hash)]
pub struct ContextRoot {
    pub fg_context: Vec<Context>,
    pub bg_context: Vec<Context>,
    pub active_interaction: InteractionContext,
}

impl ContextRoot {
    pub fn get_active(&self) -> Option<&Context> {
        self.fg_context.first()
    }
}

#[derive(Debug, Clone, Default, Hash)]
pub struct InteractionContext {
    pub tags: Vec<String>,
}

impl InteractionContext {
    pub fn from_tags(tags: impl IntoIterator<Item = String>) -> Self {
        let mut tags: Vec<String> = tags
            .into_iter()
            .filter_map(|tag| normalize_context_tag(&tag))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        tags.sort();
        Self { tags }
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        normalize_context_tag(tag)
            .as_ref()
            .is_some_and(|normalized| self.tags.contains(normalized))
    }
}

pub fn normalize_context_tag(tag: &str) -> Option<String> {
    let tag = tag.trim().to_ascii_lowercase();
    if tag.is_empty()
        || !tag
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return None;
    }
    Some(tag)
}

type Context = RawWindowHandle;
