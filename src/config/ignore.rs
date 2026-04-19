use std::{collections::HashSet, fs, path::Path};

use log::warn;
use serde::Deserialize;

use crate::domain::action::Os;

#[derive(Debug, Default, Deserialize)]
pub struct IgnoreConfig {
    #[serde(default)]
    pub applications: Vec<IgnoredApplication>,
    #[serde(default)]
    pub windows: Vec<String>,
    #[serde(default)]
    pub macos: Vec<String>,
    #[serde(default)]
    pub linux: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct IgnoredApplication {
    #[serde(rename = "name")]
    pub _name: Option<String>,
    pub windows: Option<String>,
    pub macos: Option<String>,
    pub linux: Option<String>,
}

pub fn load_ignored_process_names(path: &Path, current_os: Os) -> HashSet<String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return HashSet::new(),
        Err(err) => {
            warn!("Could not read ignore config at {:?}: {}", path, err);
            return HashSet::new();
        }
    };

    match toml::from_str::<IgnoreConfig>(&content) {
        Ok(config) => config.process_names_for(current_os),
        Err(err) => {
            warn!("Could not parse ignore config at {:?}: {}", path, err);
            HashSet::new()
        }
    }
}

impl IgnoreConfig {
    pub fn process_names_for(&self, current_os: Os) -> HashSet<String> {
        let mut process_names = HashSet::new();

        let top_level_names = match current_os {
            Os::Windows => &self.windows,
            Os::Mac => &self.macos,
            Os::Linux => &self.linux,
        };
        process_names.extend(
            top_level_names
                .iter()
                .filter_map(|name| normalize_process_name(name)),
        );

        for app in &self.applications {
            let name = match current_os {
                Os::Windows => app.windows.as_deref(),
                Os::Mac => app.macos.as_deref(),
                Os::Linux => app.linux.as_deref(),
            };

            if let Some(name) = name.and_then(normalize_process_name) {
                process_names.insert(name);
            }
        }

        process_names
    }
}

pub fn normalize_process_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ignored_windows_applications() {
        let content = r#"
windows = ["Code.exe"]

[[applications]]
name = "Windows Terminal"
windows = "WindowsTerminal.exe"

[[applications]]
name = "macOS only app"
macos = "com.example.App"
"#;

        let config: IgnoreConfig = toml::from_str(content).expect("ignore config should parse");
        let process_names = config.process_names_for(Os::Windows);

        assert!(process_names.contains("code.exe"));
        assert!(process_names.contains("windowsterminal.exe"));
        assert!(!process_names.contains("com.example.app"));
    }
}
