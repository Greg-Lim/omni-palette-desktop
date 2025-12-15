// Read the extensions file and build a application registry

use std::{fs, path::Path};

use crate::{core::registry::registry::Application, models::config::Config};

fn load_config<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn std::error::Error>> {
    // Read the file content into a string
    let content = fs::read_to_string(path)?;

    // Deserialize the TOML string into the Config struct
    let config: Config = toml::from_str(&content)?;

    Ok(config)
}

fn build_application_registry_from_toml_config(extention_config: Config) -> Application {
    todo!("build mapping");
}

#[test]
fn deserializes_inline_toml() {
    let content = r#"
version = 1

[app]
id = "chrome"
name = "Chrome"
default_priority = "Application"

[app.os]
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
