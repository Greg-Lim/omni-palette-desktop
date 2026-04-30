use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::runtime::GitHubExtensionSource,
    core::extensions::{
        catalog::{CatalogEntry, ExtensionCatalog, ExtensionKind},
        package::{validate_package_file, PackageError},
    },
    domain::action::Os,
};

const INSTALLED_STATE_FILE: &str = "installed.toml";
const MAX_CATALOG_BYTES: usize = 5 * 1024 * 1024;
const MAX_PACKAGE_BYTES: usize = 50 * 1024 * 1024;
pub const BUNDLED_SOURCE_ID: &str = "bundled";
pub const GITHUB_SOURCE_ID: &str = "github";

#[derive(Debug, Clone)]
pub struct BundledExtension {
    pub id: String,
    pub name: String,
    pub version: String,
    pub platform: Os,
    pub kind: ExtensionKind,
    pub installed_path: PathBuf,
    pub enabled: bool,
}

impl BundledExtension {
    fn to_installed_extension(&self, enabled: bool) -> InstalledExtension {
        InstalledExtension {
            id: self.id.clone(),
            version: self.version.clone(),
            platform: self.platform,
            kind: self.kind,
            source_id: BUNDLED_SOURCE_ID.to_string(),
            package_sha256: "0".repeat(64),
            enabled,
            installed_path: self.installed_path.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtensionInstallService {
    install_root: PathBuf,
}

impl ExtensionInstallService {
    pub fn new(install_root: impl AsRef<Path>) -> Self {
        Self {
            install_root: install_root.as_ref().to_path_buf(),
        }
    }

    pub fn fetch_catalog(
        &self,
        source: &GitHubExtensionSource,
    ) -> Result<ExtensionCatalog, InstallError> {
        if !source.enabled {
            return Err(InstallError::DisabledSource("github".to_string()));
        }

        let catalog_url = source.catalog_url();
        let catalog_bytes = fetch_bytes(&catalog_url, MAX_CATALOG_BYTES)?;

        Ok(ExtensionCatalog::parse(&catalog_bytes)?)
    }

    pub fn install_entry(
        &self,
        source: &GitHubExtensionSource,
        entry: &CatalogEntry,
        current_os: Os,
    ) -> Result<InstalledExtension, InstallError> {
        if entry.platform != current_os {
            return Err(InstallError::PlatformMismatch {
                expected: current_os,
                actual: entry.platform,
            });
        }
        if entry.kind != ExtensionKind::Static {
            return Err(InstallError::UnsupportedKind(entry.kind));
        }
        if !package_url_allowed(source, &entry.package_url) {
            return Err(InstallError::PackageUrlNotAllowed(
                entry.package_url.clone(),
            ));
        }

        fs::create_dir_all(&self.install_root)?;
        let download_dir = self.install_root.join(".downloads");
        fs::create_dir_all(&download_dir)?;
        let package_path = download_dir.join(format!("{}-{}.gpext", entry.id, entry.version));
        download_file(&entry.package_url, &package_path, MAX_PACKAGE_BYTES)?;

        let package = validate_package_file(&package_path, &entry.package_sha256, current_os)?;
        let installed_path = package.install_static(&self.install_root, current_os)?;
        let installed = InstalledExtension {
            id: entry.id.clone(),
            version: entry.version.clone(),
            platform: entry.platform,
            kind: entry.kind,
            source_id: GITHUB_SOURCE_ID.to_string(),
            package_sha256: entry.package_sha256.clone(),
            enabled: true,
            installed_path,
        };

        let mut state = load_installed_state(&self.install_root)?;
        state.upsert(installed.clone());
        save_installed_state(&self.install_root, &state)?;
        let _ = fs::remove_file(package_path);

        Ok(installed)
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct InstalledState {
    #[serde(default)]
    pub extensions: Vec<InstalledExtension>,
}

impl InstalledState {
    pub fn upsert(&mut self, extension: InstalledExtension) {
        if let Some(existing) = self.extensions.iter_mut().find(|existing| {
            existing.id == extension.id && existing.source_id == extension.source_id
        }) {
            *existing = extension;
        } else {
            self.extensions.push(extension);
        }

        self.extensions.sort_by(|left, right| {
            left.id
                .cmp(&right.id)
                .then_with(|| left.source_id.cmp(&right.source_id))
        });
    }

    pub fn set_enabled(&mut self, extension_id: &str, source_id: &str, enabled: bool) -> bool {
        if let Some(extension) = self
            .extensions
            .iter_mut()
            .find(|extension| extension.id == extension_id && extension.source_id == source_id)
        {
            extension.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, extension_id: &str, source_id: &str) -> Option<InstalledExtension> {
        self.extensions
            .iter()
            .position(|extension| extension.id == extension_id && extension.source_id == source_id)
            .map(|index| self.extensions.remove(index))
    }

    pub fn enabled_for(&self, extension_id: &str, source_id: &str) -> Option<bool> {
        self.extensions
            .iter()
            .find(|extension| extension.id == extension_id && extension.source_id == source_id)
            .map(|extension| extension.enabled)
    }

    pub fn disabled_bundled_extension_ids(&self) -> std::collections::HashSet<String> {
        self.extensions
            .iter()
            .filter(|extension| extension.source_id == BUNDLED_SOURCE_ID && !extension.enabled)
            .map(|extension| extension.id.clone())
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstalledExtension {
    pub id: String,
    pub version: String,
    pub platform: Os,
    pub kind: ExtensionKind,
    pub source_id: String,
    pub package_sha256: String,
    pub enabled: bool,
    pub installed_path: PathBuf,
}

#[derive(Debug)]
pub enum InstallError {
    Io(io::Error),
    Http(String),
    Catalog(crate::core::extensions::catalog::CatalogError),
    Package(PackageError),
    DisabledSource(String),
    UnsupportedKind(ExtensionKind),
    PlatformMismatch { expected: Os, actual: Os },
    PackageUrlNotAllowed(String),
    DownloadTooLarge { url: String, max_bytes: usize },
    UnsafeInstalledPath(PathBuf),
}

impl From<io::Error> for InstallError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<crate::core::extensions::catalog::CatalogError> for InstallError {
    fn from(err: crate::core::extensions::catalog::CatalogError) -> Self {
        Self::Catalog(err)
    }
}

impl From<PackageError> for InstallError {
    fn from(err: PackageError) -> Self {
        Self::Package(err)
    }
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Io(err) => write!(f, "Extension install IO error: {err}"),
            InstallError::Http(message) => {
                write!(
                    f,
                    "Extension download error: {}",
                    explain_http_error(message)
                )
            }
            InstallError::Catalog(err) => write!(f, "Extension catalog error: {err}"),
            InstallError::Package(err) => write!(f, "Extension package error: {err}"),
            InstallError::DisabledSource(source_id) => {
                write!(f, "Extension source is disabled: {source_id}")
            }
            InstallError::UnsupportedKind(kind) => {
                write!(f, "Unsupported extension kind for install: {kind:?}")
            }
            InstallError::PlatformMismatch { expected, actual } => {
                write!(
                    f,
                    "Extension platform mismatch: expected {expected:?}, found {actual:?}"
                )
            }
            InstallError::PackageUrlNotAllowed(url) => {
                write!(
                    f,
                    "Package URL is not allowed for the configured GitHub source: {url}"
                )
            }
            InstallError::DownloadTooLarge { url, max_bytes } => {
                write!(f, "Download from {url} exceeded {max_bytes} bytes")
            }
            InstallError::UnsafeInstalledPath(path) => {
                write!(
                    f,
                    "Refusing to uninstall extension path outside the install root: {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for InstallError {}

fn explain_http_error(message: &str) -> String {
    if message.contains("status code 404") {
        format!(
            "{message}. For catalog URLs, a 404 usually means the branch or catalog path is wrong, the repository is private, or the catalog has not been pushed."
        )
    } else {
        message.to_string()
    }
}

pub fn load_installed_state(install_root: &Path) -> Result<InstalledState, InstallError> {
    let path = install_root.join(INSTALLED_STATE_FILE);
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(InstalledState::default()),
        Err(err) => return Err(InstallError::Io(err)),
    };

    toml::from_str(&content)
        .map_err(|err| InstallError::Package(PackageError::InvalidManifest(err.to_string())))
}

pub fn save_installed_state(
    install_root: &Path,
    state: &InstalledState,
) -> Result<(), InstallError> {
    fs::create_dir_all(install_root)?;
    let path = install_root.join(INSTALLED_STATE_FILE);
    let staging_path = install_root.join(format!("{INSTALLED_STATE_FILE}.installing"));
    let content = toml::to_string_pretty(state)
        .map_err(|err| InstallError::Package(PackageError::InvalidManifest(err.to_string())))?;
    fs::write(&staging_path, content)?;
    fs::rename(staging_path, path)?;
    Ok(())
}

pub fn set_installed_extension_enabled(
    install_root: &Path,
    extension_id: &str,
    source_id: &str,
    enabled: bool,
) -> Result<InstalledState, InstallError> {
    let mut state = load_installed_state(install_root)?;
    if !state.set_enabled(extension_id, source_id, enabled) {
        return Err(InstallError::Package(PackageError::InvalidManifest(
            format!("Installed extension not found: {source_id}/{extension_id}"),
        )));
    }
    save_installed_state(install_root, &state)?;
    Ok(state)
}

pub fn set_bundled_extension_enabled(
    install_root: &Path,
    extension: &BundledExtension,
    enabled: bool,
) -> Result<InstalledState, InstallError> {
    let mut state = load_installed_state(install_root)?;
    state.upsert(extension.to_installed_extension(enabled));
    save_installed_state(install_root, &state)?;
    Ok(state)
}

pub fn uninstall_installed_extension(
    install_root: &Path,
    extension_id: &str,
    source_id: &str,
) -> Result<InstalledState, InstallError> {
    if source_id == BUNDLED_SOURCE_ID {
        return Err(InstallError::Package(PackageError::InvalidManifest(
            "Bundled extensions can be disabled, but not uninstalled.".to_string(),
        )));
    }

    let mut state = load_installed_state(install_root)?;
    let extension = state.remove(extension_id, source_id).ok_or_else(|| {
        InstallError::Package(PackageError::InvalidManifest(format!(
            "Installed extension not found: {source_id}/{extension_id}"
        )))
    })?;

    let installed_path = resolve_installed_file_path(install_root, &extension.installed_path)?;
    match fs::remove_file(&installed_path) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(InstallError::Io(err)),
    }
    let metadata_path = install_root.join("metadata").join(extension_id);
    if metadata_path.exists() {
        fs::remove_dir_all(metadata_path)?;
    }

    save_installed_state(install_root, &state)?;
    Ok(state)
}

fn resolve_installed_file_path(
    install_root: &Path,
    installed_path: &Path,
) -> Result<PathBuf, InstallError> {
    let install_root = install_root.canonicalize()?;
    let candidate = if installed_path.is_absolute() {
        installed_path.to_path_buf()
    } else {
        install_root.join(installed_path)
    };

    let resolved = match candidate.canonicalize() {
        Ok(path) => path,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let file_name = candidate
                .file_name()
                .ok_or_else(|| InstallError::UnsafeInstalledPath(candidate.clone()))?;
            let parent = candidate
                .parent()
                .ok_or_else(|| InstallError::UnsafeInstalledPath(candidate.clone()))?
                .canonicalize()?;
            parent.join(file_name)
        }
        Err(err) => return Err(InstallError::Io(err)),
    };

    if !resolved.starts_with(&install_root) {
        return Err(InstallError::UnsafeInstalledPath(resolved));
    }

    Ok(resolved)
}

fn fetch_bytes(url: &str, max_bytes: usize) -> Result<Vec<u8>, InstallError> {
    let response = ureq::get(url)
        .call()
        .map_err(|err| InstallError::Http(err.to_string()))?;
    let mut reader = response.into_reader().take((max_bytes + 1) as u64);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(InstallError::DownloadTooLarge {
            url: url.to_string(),
            max_bytes,
        });
    }
    Ok(bytes)
}

fn download_file(url: &str, path: &Path, max_bytes: usize) -> Result<(), InstallError> {
    let bytes = fetch_bytes(url, max_bytes)?;
    let mut file = fs::File::create(path)?;
    file.write_all(&bytes)?;
    Ok(())
}

fn package_url_allowed(source: &GitHubExtensionSource, package_url: &str) -> bool {
    let release_prefix = format!(
        "https://github.com/{}/{}/releases/download/",
        source.owner, source.repo
    );
    package_url.starts_with(&release_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed_extension(
        id: &str,
        source_id: &str,
        installed_path: PathBuf,
    ) -> InstalledExtension {
        InstalledExtension {
            id: id.to_string(),
            version: "1.0.0".to_string(),
            platform: Os::Windows,
            kind: ExtensionKind::Static,
            source_id: source_id.to_string(),
            package_sha256: "a".repeat(64),
            enabled: true,
            installed_path,
        }
    }

    #[test]
    fn upsert_replaces_existing_extension_state() {
        let mut state = InstalledState::default();
        state.upsert(InstalledExtension {
            id: "chrome_tools".to_string(),
            version: "1.0.0".to_string(),
            platform: Os::Windows,
            kind: ExtensionKind::Static,
            source_id: "official".to_string(),
            package_sha256: "a".repeat(64),
            enabled: true,
            installed_path: PathBuf::from("static/chrome_tools.toml"),
        });
        state.upsert(InstalledExtension {
            id: "chrome_tools".to_string(),
            version: "1.1.0".to_string(),
            platform: Os::Windows,
            kind: ExtensionKind::Static,
            source_id: "official".to_string(),
            package_sha256: "b".repeat(64),
            enabled: false,
            installed_path: PathBuf::from("static/chrome_tools.toml"),
        });

        assert_eq!(state.extensions.len(), 1);
        assert_eq!(state.extensions[0].version, "1.1.0");
        assert!(!state.extensions[0].enabled);
    }

    #[test]
    fn set_enabled_updates_existing_extension() {
        let mut state = InstalledState::default();
        state.upsert(InstalledExtension {
            id: "chrome_tools".to_string(),
            version: "1.0.0".to_string(),
            platform: Os::Windows,
            kind: ExtensionKind::Static,
            source_id: "official".to_string(),
            package_sha256: "a".repeat(64),
            enabled: true,
            installed_path: PathBuf::from("static/chrome_tools.toml"),
        });

        assert!(state.set_enabled("chrome_tools", "official", false));
        assert!(!state.extensions[0].enabled);
        assert!(!state.set_enabled("missing", "official", true));
    }

    #[test]
    fn remove_deletes_only_matching_extension_source() {
        let mut state = InstalledState::default();
        state.upsert(installed_extension(
            "windows",
            BUNDLED_SOURCE_ID,
            PathBuf::from("static/windows.toml"),
        ));
        state.upsert(installed_extension(
            "windows",
            GITHUB_SOURCE_ID,
            PathBuf::from("static/windows.toml"),
        ));

        let removed = state
            .remove("windows", GITHUB_SOURCE_ID)
            .expect("downloaded extension should be removed");

        assert_eq!(removed.source_id, GITHUB_SOURCE_ID);
        assert_eq!(state.extensions.len(), 1);
        assert_eq!(state.extensions[0].source_id, BUNDLED_SOURCE_ID);
    }

    #[test]
    fn uninstall_removes_static_file_and_state_entry() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let static_dir = root.path().join("static");
        let metadata_dir = root.path().join("metadata").join("chrome");
        fs::create_dir_all(&static_dir).expect("static dir should be created");
        fs::create_dir_all(&metadata_dir).expect("metadata dir should be created");
        let installed_path = static_dir.join("chrome.toml");
        fs::write(&installed_path, "version = 2").expect("extension file should be written");
        fs::write(metadata_dir.join("manifest.toml"), "").expect("manifest should be written");
        fs::write(metadata_dir.join("actions.toml"), "").expect("actions should be written");

        let mut state = InstalledState::default();
        state.upsert(installed_extension(
            "chrome",
            GITHUB_SOURCE_ID,
            installed_path.clone(),
        ));
        save_installed_state(root.path(), &state).expect("state should be saved");

        let state = uninstall_installed_extension(root.path(), "chrome", GITHUB_SOURCE_ID)
            .expect("extension should uninstall");

        assert!(state.extensions.is_empty());
        assert!(!installed_path.exists());
        assert!(!metadata_dir.exists());
        assert!(load_installed_state(root.path())
            .expect("state should reload")
            .extensions
            .is_empty());
    }

    #[test]
    fn uninstall_refuses_paths_outside_install_root() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let outside = tempfile::tempdir().expect("outside temp dir should be created");
        let outside_path = outside.path().join("chrome.toml");
        fs::write(&outside_path, "version = 2").expect("outside file should be written");

        let mut state = InstalledState::default();
        state.upsert(installed_extension(
            "chrome",
            GITHUB_SOURCE_ID,
            outside_path.clone(),
        ));
        save_installed_state(root.path(), &state).expect("state should be saved");

        let err = uninstall_installed_extension(root.path(), "chrome", GITHUB_SOURCE_ID)
            .expect_err("unsafe path should be rejected");

        assert!(matches!(err, InstallError::UnsafeInstalledPath(_)));
        assert!(outside_path.exists());
        assert_eq!(
            load_installed_state(root.path())
                .expect("state should reload")
                .extensions
                .len(),
            1
        );
    }

    #[test]
    fn upsert_keeps_distinct_sources_for_same_extension_id() {
        let mut state = InstalledState::default();
        state.upsert(InstalledExtension {
            id: "windows".to_string(),
            version: "0.1.0".to_string(),
            platform: Os::Windows,
            kind: ExtensionKind::Static,
            source_id: BUNDLED_SOURCE_ID.to_string(),
            package_sha256: "0".repeat(64),
            enabled: false,
            installed_path: PathBuf::from("static/windows.toml"),
        });
        state.upsert(InstalledExtension {
            id: "windows".to_string(),
            version: "1.0.0".to_string(),
            platform: Os::Windows,
            kind: ExtensionKind::Static,
            source_id: GITHUB_SOURCE_ID.to_string(),
            package_sha256: "1".repeat(64),
            enabled: true,
            installed_path: PathBuf::from("static/windows.toml"),
        });

        assert_eq!(state.extensions.len(), 2);
        assert_eq!(state.enabled_for("windows", BUNDLED_SOURCE_ID), Some(false));
        assert_eq!(state.enabled_for("windows", GITHUB_SOURCE_ID), Some(true));
    }

    #[test]
    fn package_url_must_match_configured_github_repo() {
        let source = GitHubExtensionSource {
            owner: "Greg-Lim".to_string(),
            repo: "omni-palette-desktop".to_string(),
            branch: "master".to_string(),
            catalog_path: "extensions/registry/catalog.v1.json".to_string(),
            enabled: true,
        };

        assert!(package_url_allowed(
            &source,
            "https://github.com/Greg-Lim/omni-palette-desktop/releases/download/chrome-v1/chrome.gpext"
        ));
        assert!(!package_url_allowed(
            &source,
            "https://github.com/other/omni-palette-desktop/releases/download/chrome-v1/chrome.gpext"
        ));
    }

    #[test]
    fn set_bundled_extension_enabled_persists_wasm_plugin_state() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let bundled_plugin = BundledExtension {
            id: "ahk_agent".to_string(),
            name: "AHK".to_string(),
            version: "0.1.0".to_string(),
            platform: Os::Windows,
            kind: ExtensionKind::WasmPlugin,
            installed_path: PathBuf::from("plugins/ahk_agent/plugin.toml"),
            enabled: true,
        };

        let state = set_bundled_extension_enabled(root.path(), &bundled_plugin, false)
            .expect("bundled plugin state should persist");

        assert_eq!(state.enabled_for("ahk_agent", BUNDLED_SOURCE_ID), Some(false));
        assert_eq!(state.extensions.len(), 1);
        assert_eq!(state.extensions[0].kind, ExtensionKind::WasmPlugin);
        assert_eq!(
            state.extensions[0].installed_path,
            PathBuf::from("plugins/ahk_agent/plugin.toml")
        );
    }

    #[test]
    fn http_404_error_explains_likely_catalog_causes() {
        let err = InstallError::Http(
            "https://raw.githubusercontent.com/Greg-Lim/omni-palette-desktop/main/catalog.v1.json: status code 404"
                .to_string(),
        );
        let message = err.to_string();

        assert!(message.contains("branch or catalog path is wrong"));
        assert!(message.contains("repository is private"));
        assert!(message.contains("catalog has not been pushed"));
    }
}
