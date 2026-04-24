use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::domain::action::Os;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtensionCatalog {
    pub schema_version: u32,
    pub generated_at: Option<String>,
    pub expires_at_unix: Option<u64>,
    #[serde(default)]
    pub entries: Vec<CatalogEntry>,
}

impl ExtensionCatalog {
    pub fn parse(catalog_bytes: &[u8]) -> Result<Self, CatalogError> {
        let catalog: Self = serde_json::from_slice(catalog_bytes)?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), CatalogError> {
        if self.schema_version != 1 {
            return Err(CatalogError::UnsupportedSchema(self.schema_version));
        }

        if let Some(expires_at) = self.expires_at_unix {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| CatalogError::SystemClockBeforeUnixEpoch)?
                .as_secs();
            if expires_at <= now {
                return Err(CatalogError::Expired);
            }
        }

        for entry in &self.entries {
            entry.validate()?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub platform: Os,
    pub kind: ExtensionKind,
    pub package_url: String,
    pub package_sha256: String,
    pub size_bytes: Option<u64>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub min_app_version: Option<String>,
}

impl CatalogEntry {
    pub fn validate(&self) -> Result<(), CatalogError> {
        validate_extension_id(&self.id)?;
        validate_version(&self.version)?;
        validate_sha256_hex(&self.package_sha256)?;

        if let Some(min_app_version) = &self.min_app_version {
            validate_version(min_app_version)?;
        }

        if self.package_url.trim().is_empty() {
            return Err(CatalogError::InvalidEntry(format!(
                "{} package_url is empty",
                self.id
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionKind {
    Static,
    WasmPlugin,
}

#[derive(Debug)]
pub enum CatalogError {
    Json(serde_json::Error),
    UnsupportedSchema(u32),
    Expired,
    SystemClockBeforeUnixEpoch,
    InvalidEntry(String),
}

impl From<serde_json::Error> for CatalogError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CatalogError::Json(err) => write!(f, "Could not parse catalog JSON: {err}"),
            CatalogError::UnsupportedSchema(version) => {
                write!(f, "Unsupported catalog schema version: {version}")
            }
            CatalogError::Expired => write!(f, "Catalog has expired"),
            CatalogError::SystemClockBeforeUnixEpoch => {
                write!(f, "System clock is before the Unix epoch")
            }
            CatalogError::InvalidEntry(message) => write!(f, "Invalid catalog entry: {message}"),
        }
    }
}

impl std::error::Error for CatalogError {}

pub fn validate_extension_id(id: &str) -> Result<(), CatalogError> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
        && id.chars().next().is_some_and(|ch| ch.is_ascii_lowercase());

    if valid {
        Ok(())
    } else {
        Err(CatalogError::InvalidEntry(format!(
            "invalid extension id: {id}"
        )))
    }
}

fn validate_version(version: &str) -> Result<(), CatalogError> {
    semver::Version::parse(version)
        .map(|_| ())
        .map_err(|err| CatalogError::InvalidEntry(format!("invalid semver {version}: {err}")))
}

pub fn validate_sha256_hex(hash: &str) -> Result<(), CatalogError> {
    let valid = hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(CatalogError::InvalidEntry(format!(
            "invalid sha256 hex: {hash}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_extension_id() {
        assert!(validate_extension_id("chrome_tools").is_ok());
        assert!(validate_extension_id("ChromeTools").is_err());
        assert!(validate_extension_id("1chrome").is_err());
        assert!(validate_extension_id("../chrome").is_err());
    }

    #[test]
    fn validates_catalog_entries() {
        let catalog = ExtensionCatalog {
            schema_version: 1,
            generated_at: None,
            expires_at_unix: None,
            entries: vec![CatalogEntry {
                id: "chrome_tools".to_string(),
                name: "Chrome Tools".to_string(),
                version: "1.0.0".to_string(),
                platform: Os::Windows,
                kind: ExtensionKind::Static,
                package_url: "https://example.com/chrome_tools-1.0.0.gpext".to_string(),
                package_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
                size_bytes: Some(100),
                publisher: None,
                description: None,
                license: None,
                homepage: None,
                repository: None,
                keywords: Vec::new(),
                min_app_version: None,
            }],
        };

        assert!(catalog.validate().is_ok());
    }

    #[test]
    fn parses_catalog_json_without_signature() {
        let catalog_json =
            br#"{"schema_version":1,"generated_at":null,"expires_at_unix":null,"entries":[]}"#;

        let catalog = ExtensionCatalog::parse(catalog_json).expect("valid catalog should parse");
        assert_eq!(catalog.schema_version, 1);

        let err = ExtensionCatalog::parse(br#"{"schema_version":2,"entries":[]}"#)
            .expect_err("unsupported schema should fail validation");

        assert!(err
            .to_string()
            .contains("Unsupported catalog schema version"));
    }
}
