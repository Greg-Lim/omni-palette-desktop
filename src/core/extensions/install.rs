#![allow(dead_code)]
// Backend for upcoming palette install/update commands. It is covered by unit tests,
// but is not invoked by the runtime UI yet.

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

    pub fn install_root(&self) -> &Path {
        &self.install_root
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
        let signature_url = source.signature_url();
        let signature = String::from_utf8(fetch_bytes(&signature_url, 64 * 1024)?)
            .map_err(|err| InstallError::InvalidUtf8(err.to_string()))?;

        Ok(ExtensionCatalog::parse_verified(
            &catalog_bytes,
            &signature,
            &source.public_key,
        )?)
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
            source_id: "github".to_string(),
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
        if let Some(existing) = self
            .extensions
            .iter_mut()
            .find(|existing| existing.id == extension.id)
        {
            *existing = extension;
        } else {
            self.extensions.push(extension);
        }

        self.extensions
            .sort_by(|left, right| left.id.cmp(&right.id));
    }

    pub fn enabled_extension_ids(&self) -> Vec<&str> {
        self.extensions
            .iter()
            .filter(|extension| extension.enabled)
            .map(|extension| extension.id.as_str())
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
    InvalidUtf8(String),
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
            InstallError::Http(message) => write!(f, "Extension download error: {message}"),
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
            InstallError::InvalidUtf8(message) => write!(f, "Invalid UTF-8: {message}"),
        }
    }
}

impl std::error::Error for InstallError {}

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
        assert!(state.enabled_extension_ids().is_empty());
    }

    #[test]
    fn package_url_must_match_configured_github_repo() {
        let source = GitHubExtensionSource {
            owner: "limgr".to_string(),
            repo: "global-palette-extensions".to_string(),
            branch: "main".to_string(),
            catalog_path: "catalog.v1.json".to_string(),
            public_key: "abc".to_string(),
            enabled: true,
        };

        assert!(package_url_allowed(
            &source,
            "https://github.com/limgr/global-palette-extensions/releases/download/chrome-v1/chrome.gpext"
        ));
        assert!(!package_url_allowed(
            &source,
            "https://github.com/other/global-palette-extensions/releases/download/chrome-v1/chrome.gpext"
        ));
    }
}
