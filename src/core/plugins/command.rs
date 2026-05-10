use serde::Deserialize;

use crate::{
    config::extension::{ActionWhenConfig, CommandBinding},
    core::plugins::manifest::PluginAppConfig,
    domain::action::{ActionContextCondition, CommandPriority, FocusState},
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
    pub when: ActionContextCondition,
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
    when: Option<ActionWhenConfig>,
    shortcut_text: Option<String>,
    cmd: Option<CommandBinding>,
}

impl RawCommandDescriptor {
    pub(crate) fn into_plugin_command(
        self,
        app: Option<&PluginAppConfig>,
    ) -> Result<PluginCommand, String> {
        let mut tags = app
            .and_then(|app| app.default_tags.clone())
            .unwrap_or_default();
        tags.extend(self.tags);
        tags.sort();
        tags.dedup();
        let when_any = self
            .when
            .as_ref()
            .map(|when| when.any.as_slice())
            .or_else(|| {
                app.and_then(|app| app.default_when.as_ref().map(|when| when.any.as_slice()))
            });

        Ok(PluginCommand {
            id: self.id,
            name: self.name,
            priority: self.priority.unwrap_or_default(),
            focus_state: self
                .focus_state
                .or_else(|| app.and_then(|app| app.default_focus_state))
                .unwrap_or(FocusState::Global),
            when: ActionContextCondition::from_optional_any(when_any)?,
            favorite: self.favorite,
            tags,
            shortcut_text: self.shortcut_text,
            cmd: self.cmd,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::extension::ActionWhenConfig,
        domain::action::{ActionContextCondition, FocusState},
    };

    #[test]
    fn app_default_when_applies_to_plugin_command_without_command_when() {
        let raw: RawCommandDescriptor = serde_json::from_str(
            r#"{"id":"type_date","name":"Type date","shortcut_text":"{D} {MMM}"}"#,
        )
        .expect("command should parse");
        let app = PluginAppConfig {
            default_focus_state: Some(FocusState::Global),
            default_tags: None,
            default_when: Some(ActionWhenConfig {
                any: vec!["ui.text_input".to_string()],
            }),
        };

        let command = raw
            .into_plugin_command(Some(&app))
            .expect("command should build");

        assert_eq!(
            command.when,
            ActionContextCondition {
                any: vec!["ui.text_input".to_string()]
            }
        );
    }

    #[test]
    fn command_when_overrides_app_default_when() {
        let raw: RawCommandDescriptor = serde_json::from_str(
            r#"{"id":"type_date","name":"Type date","when":{"any":["ppt.selection.text"]}}"#,
        )
        .expect("command should parse");
        let app = PluginAppConfig {
            default_focus_state: None,
            default_tags: None,
            default_when: Some(ActionWhenConfig {
                any: vec!["ui.text_input".to_string()],
            }),
        };

        let command = raw
            .into_plugin_command(Some(&app))
            .expect("command should build");

        assert_eq!(
            command.when,
            ActionContextCondition {
                any: vec!["ppt.selection.text".to_string()]
            }
        );
    }

    #[test]
    fn explicitly_empty_command_when_is_rejected() {
        let raw: RawCommandDescriptor =
            serde_json::from_str(r#"{"id":"type_date","name":"Type date","when":{"any":[]}}"#)
                .expect("command should parse");

        let err = raw
            .into_plugin_command(None)
            .expect_err("empty when.any should fail");

        assert!(err.contains("when.any"));
    }
}
