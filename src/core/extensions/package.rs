use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use zip::ZipArchive;

use crate::{
    config::extension::{
        ActionsMetadataConfig, PackageManifestConfig, PlatformImplementationConfig,
    },
    core::extensions::catalog::{validate_extension_id, validate_sha256_hex, ExtensionKind},
    domain::action::Os,
};

const PACKAGE_MANIFEST_NAME: &str = "manifest.toml";
const PACKAGE_ACTIONS_NAME: &str = "actions.toml";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtensionPackageManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: ExtensionKind,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub min_app_version: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

impl ExtensionPackageManifest {
    pub fn validate(&self) -> Result<(), PackageError> {
        if self.schema_version != 1 {
            return Err(PackageError::UnsupportedSchema(self.schema_version));
        }
        validate_extension_id(&self.id)
            .map_err(|err| PackageError::InvalidManifest(err.to_string()))?;
        semver::Version::parse(&self.version).map_err(|err| {
            PackageError::InvalidManifest(format!(
                "invalid package version {}: {err}",
                self.version
            ))
        })?;

        if let Some(min_app_version) = &self.min_app_version {
            semver::Version::parse(min_app_version).map_err(|err| {
                PackageError::InvalidManifest(format!(
                    "invalid minimum app version {min_app_version}: {err}"
                ))
            })?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct ValidatedPackage {
    pub manifest: ExtensionPackageManifest,
    pub platform: Os,
    temp_dir: TempDir,
}

impl ValidatedPackage {
    pub fn install_static(
        self,
        install_root: &Path,
        current_os: Os,
    ) -> Result<PathBuf, PackageError> {
        if self.manifest.kind != ExtensionKind::Static {
            return Err(PackageError::UnsupportedKind(self.manifest.kind));
        }
        if self.platform != current_os {
            return Err(PackageError::PlatformMismatch {
                expected: current_os,
                actual: self.platform,
            });
        }

        let source_static_path = self
            .temp_dir
            .path()
            .join("static")
            .join(format!("{}.toml", self.manifest.id));
        let static_content = fs::read_to_string(&source_static_path)?;
        let platform_config: PlatformImplementationConfig = toml::from_str(&static_content)?;
        if platform_config.platform != self.platform {
            return Err(PackageError::InvalidManifest(format!(
                "package platform {:?} does not match static extension platform {:?}",
                self.platform, platform_config.platform
            )));
        }

        let destination_dir = install_root.join("static");
        fs::create_dir_all(&destination_dir)?;
        let destination_path = destination_dir.join(format!("{}.toml", self.manifest.id));
        let staging_path = destination_dir.join(format!("{}.toml.installing", self.manifest.id));
        fs::write(&staging_path, static_content)?;
        fs::rename(&staging_path, &destination_path)?;

        let metadata_dir = install_root.join("metadata").join(&self.manifest.id);
        fs::create_dir_all(&metadata_dir)?;
        fs::copy(
            self.temp_dir.path().join(PACKAGE_MANIFEST_NAME),
            metadata_dir.join(PACKAGE_MANIFEST_NAME),
        )?;
        fs::copy(
            self.temp_dir.path().join(PACKAGE_ACTIONS_NAME),
            metadata_dir.join(PACKAGE_ACTIONS_NAME),
        )?;

        Ok(destination_path)
    }
}

#[derive(Debug)]
pub enum PackageError {
    Io(io::Error),
    Zip(zip::result::ZipError),
    Toml(toml::de::Error),
    UnsupportedSchema(u32),
    UnsupportedKind(ExtensionKind),
    PlatformMismatch { expected: Os, actual: Os },
    HashMismatch { expected: String, actual: String },
    InvalidManifest(String),
    UnsafeArchivePath(String),
    MissingManifest,
}

impl From<io::Error> for PackageError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<zip::result::ZipError> for PackageError {
    fn from(err: zip::result::ZipError) -> Self {
        Self::Zip(err)
    }
}

impl From<toml::de::Error> for PackageError {
    fn from(err: toml::de::Error) -> Self {
        Self::Toml(err)
    }
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageError::Io(err) => write!(f, "Package IO error: {err}"),
            PackageError::Zip(err) => write!(f, "Package zip error: {err}"),
            PackageError::Toml(err) => write!(f, "Package TOML error: {err}"),
            PackageError::UnsupportedSchema(version) => {
                write!(f, "Unsupported package schema version: {version}")
            }
            PackageError::UnsupportedKind(kind) => {
                write!(f, "Unsupported package extension kind: {kind:?}")
            }
            PackageError::PlatformMismatch { expected, actual } => {
                write!(
                    f,
                    "Package platform mismatch: expected {expected:?}, found {actual:?}"
                )
            }
            PackageError::HashMismatch { expected, actual } => {
                write!(
                    f,
                    "Package hash mismatch: expected {expected}, calculated {actual}"
                )
            }
            PackageError::InvalidManifest(message) => {
                write!(f, "Invalid package manifest: {message}")
            }
            PackageError::UnsafeArchivePath(path) => {
                write!(f, "Package contains unsafe archive path: {path}")
            }
            PackageError::MissingManifest => write!(f, "Package is missing manifest.toml"),
        }
    }
}

impl std::error::Error for PackageError {}

pub fn validate_package_file(
    package_path: &Path,
    expected_sha256: &str,
    current_os: Os,
) -> Result<ValidatedPackage, PackageError> {
    validate_sha256_hex(expected_sha256)
        .map_err(|err| PackageError::InvalidManifest(err.to_string()))?;
    let actual_sha256 = sha256_file(package_path)?;
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(PackageError::HashMismatch {
            expected: expected_sha256.to_string(),
            actual: actual_sha256,
        });
    }

    extract_and_validate(package_path, current_os)
}

pub fn extract_and_validate(
    package_path: &Path,
    current_os: Os,
) -> Result<ValidatedPackage, PackageError> {
    let file = fs::File::open(package_path)?;
    let mut archive = ZipArchive::new(file)?;
    let temp_dir = tempfile::tempdir()?;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let archive_name = file.name().to_string();
        let relative_path = safe_archive_path(&archive_name)
            .ok_or(PackageError::UnsafeArchivePath(archive_name))?;
        let output_path = temp_dir.path().join(relative_path);

        if file.is_dir() {
            fs::create_dir_all(&output_path)?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output_file = fs::File::create(&output_path)?;
        io::copy(&mut file, &mut output_file)?;
    }

    let manifest_path = temp_dir.path().join(PACKAGE_MANIFEST_NAME);
    if !manifest_path.is_file() {
        return Err(PackageError::MissingManifest);
    }
    let manifest_content = fs::read_to_string(manifest_path)?;
    let manifest: ExtensionPackageManifest = toml::from_str(&manifest_content)?;
    manifest.validate()?;
    if manifest.kind == ExtensionKind::Static {
        let static_path = temp_dir
            .path()
            .join("static")
            .join(format!("{}.toml", manifest.id));
        if !static_path.is_file() {
            return Err(PackageError::InvalidManifest(format!(
                "static package is missing static/{}.toml",
                manifest.id
            )));
        }
        let actions_path = temp_dir.path().join(PACKAGE_ACTIONS_NAME);
        if !actions_path.is_file() {
            return Err(PackageError::InvalidManifest(
                "static package is missing actions.toml".to_string(),
            ));
        }
        validate_split_package_files(&manifest, &actions_path, &static_path, current_os)?;
    }

    Ok(ValidatedPackage {
        manifest,
        platform: current_os,
        temp_dir,
    })
}

pub fn sha256_file(path: &Path) -> Result<String, PackageError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn safe_archive_path(path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() {
        return None;
    }

    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    (!safe.as_os_str().is_empty()).then_some(safe)
}

pub fn validate_split_package_files(
    manifest: &ExtensionPackageManifest,
    actions_path: &Path,
    static_path: &Path,
    current_os: Os,
) -> Result<(), PackageError> {
    let manifest_config = PackageManifestConfig {
        schema_version: manifest.schema_version,
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        kind: match manifest.kind {
            ExtensionKind::Static => "static".to_string(),
            ExtensionKind::WasmPlugin => "wasm_plugin".to_string(),
        },
        publisher: manifest.publisher.clone(),
        description: manifest.description.clone(),
        license: manifest.license.clone(),
        homepage: manifest.homepage.clone(),
        repository: manifest.repository.clone(),
        keywords: manifest.keywords.clone(),
        min_app_version: manifest.min_app_version.clone(),
        permissions: manifest.permissions.clone(),
    };
    let actions_metadata: ActionsMetadataConfig = toml::from_str(
        &fs::read_to_string(actions_path)
            .map_err(|err| PackageError::InvalidManifest(err.to_string()))?,
    )?;
    let platform_config: PlatformImplementationConfig = toml::from_str(
        &fs::read_to_string(static_path)
            .map_err(|err| PackageError::InvalidManifest(err.to_string()))?,
    )?;
    if platform_config.platform != current_os {
        return Err(PackageError::PlatformMismatch {
            expected: current_os,
            actual: platform_config.platform,
        });
    }

    crate::core::extensions::extensions::resolved_config_from_split(
        manifest_config,
        actions_metadata,
        platform_config,
    )
    .map_err(PackageError::InvalidManifest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::extensions::catalog::ExtensionCatalog;
    use std::io::Write;
    use zip::{write::SimpleFileOptions, ZipWriter};

    #[test]
    fn rejects_unsafe_archive_paths() {
        assert!(safe_archive_path("manifest.toml").is_some());
        assert!(safe_archive_path("static/chrome.toml").is_some());
        assert!(safe_archive_path("../chrome.toml").is_none());
        assert!(safe_archive_path("/chrome.toml").is_none());
        assert!(safe_archive_path("static/../../chrome.toml").is_none());
    }

    #[test]
    fn validates_and_installs_static_package() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let package_path = root.path().join("chrome_tools-1.0.0.gpext");
        let file = fs::File::create(&package_path).expect("package should be created");
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        zip.start_file(PACKAGE_MANIFEST_NAME, options)
            .expect("manifest should start");
        zip.write_all(
            br#"schema_version = 1
id = "chrome_tools"
name = "Chrome Tools"
version = "1.0.0"
kind = "static"
description = "Chrome tools."
repository = "https://github.com/Greg-Lim/omni-palette-desktop"
keywords = ["chrome"]
"#,
        )
        .expect("manifest should be written");
        zip.start_file("actions.toml", options)
            .expect("actions should start");
        zip.write_all(
            br#"schema_version = 1

[app]
default_focus_state = "global"

[actions]

[actions.new_tab]
name = "New tab"
"#,
        )
        .expect("actions should be written");
        zip.start_file("static/chrome_tools.toml", options)
            .expect("static file should start");
        zip.write_all(
            br#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.new_tab]
cmd = { mods = ["ctrl"], key = "T" }
"#,
        )
        .expect("static file should be written");
        zip.finish().expect("zip should finish");

        let hash = sha256_file(&package_path).expect("package hash should compute");
        let err = validate_package_file(&package_path, &hash, Os::Mac)
            .expect_err("wrong OS package should fail");
        assert!(matches!(err, PackageError::PlatformMismatch { .. }));

        let package = validate_package_file(&package_path, &hash, Os::Windows)
            .expect("package should validate");
        let installed_path = package
            .install_static(root.path(), Os::Windows)
            .expect("package should install");

        assert_eq!(installed_path, root.path().join("static/chrome_tools.toml"));
        assert!(installed_path.is_file());
        assert!(root
            .path()
            .join("metadata/chrome_tools/manifest.toml")
            .is_file());
        assert!(root
            .path()
            .join("metadata/chrome_tools/actions.toml")
            .is_file());
    }

    #[test]
    fn registry_package_sources_match_static_configs() {
        for package_id in ["chrome", "file_explorer", "powerpoint", "windows"] {
            let extension_root = Path::new("extensions")
                .join("registry")
                .join("packages")
                .join(package_id);
            let manifest_path = extension_root.join(PACKAGE_MANIFEST_NAME);
            let actions_path = extension_root.join(PACKAGE_ACTIONS_NAME);
            let static_path = extension_root
                .join("windows")
                .join("static")
                .join(format!("{package_id}.toml"));

            let manifest: ExtensionPackageManifest = toml::from_str(
                &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
            )
            .expect("manifest should parse");
            manifest.validate().expect("manifest should validate");

            validate_split_package_files(&manifest, &actions_path, &static_path, Os::Windows)
                .expect("split package source should validate");
            let config = crate::core::extensions::extensions::load_config(&static_path)
                .expect("split config should resolve");

            assert_eq!(manifest.id, config.app.id);
            assert_eq!(manifest.name, config.app.name);
            assert_eq!(config.platform, Os::Windows);
            assert_eq!(manifest.kind, ExtensionKind::Static);
        }
    }

    #[test]
    fn catalog_entries_match_registry_manifests() {
        let catalog = ExtensionCatalog::parse(
            &fs::read(
                Path::new("extensions")
                    .join("registry")
                    .join("catalog.v1.json"),
            )
            .expect("catalog should be readable"),
        )
        .expect("catalog should parse");
        let catalog_ids = catalog
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(catalog_ids, vec!["chrome", "file_explorer", "powerpoint"]);

        for entry in &catalog.entries {
            let manifest_path = Path::new("extensions")
                .join("registry")
                .join("packages")
                .join(&entry.id)
                .join(PACKAGE_MANIFEST_NAME);
            let manifest: ExtensionPackageManifest = toml::from_str(
                &fs::read_to_string(manifest_path).expect("manifest should be readable"),
            )
            .expect("manifest should parse");

            assert_eq!(entry.id, manifest.id);
            assert_eq!(entry.name, manifest.name);
            assert_eq!(entry.version, manifest.version);
            assert_eq!(entry.kind, manifest.kind);
        }
    }

    #[test]
    fn generated_registry_artifacts_validate_when_present() {
        let catalog = ExtensionCatalog::parse(
            &fs::read(
                Path::new("extensions")
                    .join("registry")
                    .join("catalog.v1.json"),
            )
            .expect("catalog should be readable"),
        )
        .expect("catalog should parse");

        for entry in catalog.entries {
            let artifact_path = Path::new("target")
                .join("extensions")
                .join(format!("{}-{}-windows.gpext", entry.id, entry.version));
            if artifact_path.exists() {
                validate_package_file(&artifact_path, &entry.package_sha256, entry.platform)
                    .expect("generated package artifact should validate");
            }
        }
    }
}
