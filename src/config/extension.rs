use serde::{
    de::value::StrDeserializer, de::Error as DeError, Deserialize, Deserializer, Serialize,
};
use std::collections::HashMap;

use crate::domain::{
    action::{CommandPriority, FocusState, Os},
    hotkey::Key,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub platform: Os,
    pub app: AppConfig,
    pub actions: HashMap<String, ActionConfig>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub id: String,
    pub name: String,
    pub process_name: String,
    pub default_focus_state: Option<FocusState>,
    pub default_tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ActionConfig {
    pub name: String,
    pub focus_state: Option<FocusState>,
    pub when: Option<ActionWhenConfig>,
    #[serde(alias = "action_priority")]
    pub priority: Option<CommandPriority>,
    pub tags: Option<Vec<String>>,
    pub favorite: Option<bool>,
    pub cmd: CommandBinding,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ActionWhenConfig {
    #[serde(default)]
    pub any: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum CommandBinding {
    Shortcut(KeyChord),
    Sequence(KeySequence),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct KeyChord {
    #[serde(default)]
    pub mods: Vec<Modifier>,
    pub key: Key,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct KeySequence {
    pub sequence: Vec<KeySequenceStepConfig>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct KeySequenceStepConfig {
    #[serde(default)]
    pub mods: Vec<Modifier>,
    pub key: SequenceKeyConfig,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SequenceKeyConfig {
    Ctrl,
    Shift,
    Alt,
    Win,
    Key(Key),
}

impl<'de> Deserialize<'de> for SequenceKeyConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let key_name = value.trim();
        match key_name {
            "Ctrl" => Ok(Self::Ctrl),
            "Shift" => Ok(Self::Shift),
            "Alt" => Ok(Self::Alt),
            "Win" => Ok(Self::Win),
            _ => Key::deserialize(StrDeserializer::<D::Error>::new(key_name))
                .map(Self::Key)
                .map_err(|_| {
                    D::Error::custom(format!(
                        "unknown sequence key '{value}'; use a known key name, not literal text"
                    ))
                }),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Cmd,
    Win,
    Fn,
    // Meta,
}
