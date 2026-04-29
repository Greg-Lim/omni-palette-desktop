use serde::Deserialize;

use crate::{
    config::extension::CommandBinding,
    core::plugins::manifest::PluginAppConfig,
    domain::action::{CommandPriority, FocusState},
};

#[derive(Debug, Clone)]
pub struct PluginApplication {
    pub plugin_id: String,
    pub name: String,
    pub process_name: String,
    pub commands: Vec<PluginCommand>,
}

#[derive(Debug, Clone)]
pub struct PluginCommand {
    pub id: String,
    pub name: String,
    pub priority: CommandPriority,
    pub focus_state: FocusState,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub shortcut_text: Option<String>,
    pub cmd: Option<CommandBinding>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCommandDescriptor {
    id: String,
    name: String,
    priority: Option<CommandPriority>,
    focus_state: Option<FocusState>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    favorite: bool,
    shortcut_text: Option<String>,
    cmd: Option<CommandBinding>,
}

impl RawCommandDescriptor {
    pub(crate) fn into_plugin_command(self, app: Option<&PluginAppConfig>) -> PluginCommand {
        let mut tags = app
            .and_then(|app| app.default_tags.clone())
            .unwrap_or_default();
        tags.extend(self.tags);
        tags.sort();
        tags.dedup();

        PluginCommand {
            id: self.id,
            name: self.name,
            priority: self.priority.unwrap_or_default(),
            focus_state: self
                .focus_state
                .or_else(|| app.and_then(|app| app.default_focus_state))
                .unwrap_or(FocusState::Global),
            favorite: self.favorite,
            tags,
            shortcut_text: self.shortcut_text,
            cmd: self.cmd,
        }
    }
}
