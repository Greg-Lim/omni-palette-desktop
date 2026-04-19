use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtensionCatalog {
    pub schema_version: u32,
    pub generated_at: Option<String>,
    pub expires_at_unix: Option<u64>,
    #[serde(default)]
    pub entries: Vec<CatalogEntry>,
}

impl ExtensionCatalog {
    pub fn parse_verified(
        catalog_bytes: &[u8],
        signature_base64: &str,
        public_key_base64: &str,
    ) -> Result<Self, CatalogError> {
        verify_signature(catalog_bytes, signature_base64, public_key_base64)?;
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
    #[serde(default)]
    pub platforms: Vec<String>,
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
    DecodeBase64(base64::DecodeError),
    Signature(ed25519_dalek::SignatureError),
    Json(serde_json::Error),
    UnsupportedSchema(u32),
    Expired,
    SystemClockBeforeUnixEpoch,
    InvalidEntry(String),
}

impl From<base64::DecodeError> for CatalogError {
    fn from(err: base64::DecodeError) -> Self {
        Self::DecodeBase64(err)
    }
}

impl From<ed25519_dalek::SignatureError> for CatalogError {
    fn from(err: ed25519_dalek::SignatureError) -> Self {
        Self::Signature(err)
    }
}

impl From<serde_json::Error> for CatalogError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CatalogError::DecodeBase64(err) => write!(f, "Could not decode base64: {err}"),
            CatalogError::Signature(err) => write!(f, "Catalog signature error: {err}"),
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

pub fn verify_signature(
    bytes: &[u8],
    signature_base64: &str,
    public_key_base64: &str,
) -> Result<(), CatalogError> {
    let public_key_bytes = STANDARD.decode(public_key_base64.trim())?;
    let signature_bytes = STANDARD.decode(signature_base64.trim())?;
    let public_key_array: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| CatalogError::InvalidEntry("public key must be 32 bytes".to_string()))?;
    let signature_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| CatalogError::InvalidEntry("signature must be 64 bytes".to_string()))?;

    let public_key = VerifyingKey::from_bytes(&public_key_array)?;
    let signature = Signature::from_bytes(&signature_array);
    public_key.verify(bytes, &signature)?;
    Ok(())
}

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
    use ed25519_dalek::{Signer, SigningKey};

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
                platforms: Vec::new(),
                min_app_version: None,
            }],
        };

        assert!(catalog.validate().is_ok());
    }

    #[test]
    fn parses_catalog_only_with_valid_signature() {
        let catalog_json =
            br#"{"schema_version":1,"generated_at":null,"expires_at_unix":null,"entries":[]}"#;
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let verifying_key = signing_key.verifying_key();
        let signature = signing_key.sign(catalog_json);
        let signature_base64 = STANDARD.encode(signature.to_bytes());
        let public_key_base64 = STANDARD.encode(verifying_key.to_bytes());

        let catalog =
            ExtensionCatalog::parse_verified(catalog_json, &signature_base64, &public_key_base64)
                .expect("valid signature should parse");
        assert_eq!(catalog.schema_version, 1);

        let err = ExtensionCatalog::parse_verified(
            br#"{"schema_version":2,"entries":[]}"#,
            &signature_base64,
            &public_key_base64,
        )
        .expect_err("tampered catalog should fail signature verification");

        assert!(err.to_string().contains("signature"));
    }
}
