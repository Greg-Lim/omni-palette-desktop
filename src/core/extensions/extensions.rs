// Read the extensions file and build a application registry

use std::{fs, path::Path};

use crate::config::extension::Config;

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config, String> {
    // 1. Read file. If it fails, convert the io::Error to your String error and return early.
    let content = fs::read_to_string(path).map_err(|e| format!("Could not read file: {e}"))?;

    // 2. Parse TOML. If it fails, convert the toml::de::Error to String and return early.
    let config: Config =
        toml::from_str(&content).map_err(|e| format!("Could not parse config: {e}"))?;

    Ok(config)
}

#[test]
fn deserializes_inline_toml() {
    let content = r#"
version = 1

[app]
id = "chrome"
name = "Chrome"

[app.app_os_name]
windows = "chrome.exe"
macos = "com.google.Chrome"

[actions]

[actions.new_tab]
name = "New tab"
focus_state = "focused"
cmd.windows = { mods = ["ctrl"], key = "t" }
cmd.macos = { mods = ["cmd"], key = "t" }
"#;

    let cfg: Config = toml::from_str(content).expect("should deserialize");
    assert_eq!(cfg.app.id, "chrome");
    assert!(cfg.actions.contains_key("new_tab"));
    // println!("{cfg:?}")
}

#[test]
fn rejects_app_level_priority() {
    let content = r#"
version = 1

[app]
id = "chrome"
name = "Chrome"
default_focus_state = "focused"
default_priority = "normal"

[app.app_os_name]
windows = "chrome.exe"

[actions]
"#;

    let err = toml::from_str::<Config>(content).expect_err("app priority should not deserialize");
    assert!(err.to_string().contains("default_priority"));
}
