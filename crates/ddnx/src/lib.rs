use std::collections::BTreeSet;

use next_domain::{AssetId, SCHEMA_VERSION as NEXT_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod archive;
mod equivalence;
mod hydration;
mod projection;

pub use archive::{PackageIoError, ReadPackage, read_package, write_package_to_vec};
pub use equivalence::{PersistenceComparison, PersistenceComparisonError, compare_persistence};
pub use hydration::HydrationError;
pub use projection::{
    PackageAssetDescriptor, PackageAssetKind, PreparationError, PreparedAssetBlob, PreparedPackage,
    prepare_package,
};

pub const PACKAGE_MAGIC: &str = "ddnx";
pub const PACKAGE_VERSION: u32 = 1;
pub const DOCUMENT_VERSION: u32 = 1;
pub const MANIFEST_PATH: &str = "manifest.json";
pub const DOCUMENT_PATH: &str = "document.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageLimits {
    pub max_entries: usize,
    pub max_manifest_bytes: u64,
    pub max_document_bytes: u64,
    pub max_asset_bytes: u64,
    pub max_total_uncompressed_bytes: u64,
}

impl Default for PackageLimits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_manifest_bytes: 1024 * 1024,
            max_document_bytes: 64 * 1024 * 1024,
            max_asset_bytes: 256 * 1024 * 1024,
            max_total_uncompressed_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Document,
    TemplatePalette,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub package_version: u32,
    pub document_version: u32,
    pub next_schema_version: u32,
    pub artifact_kind: ArtifactKind,
    pub document_path: String,
    pub document_sha256: String,
    pub document_bytes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<ManifestAsset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previews: Vec<ManifestPreview>,
}

impl Manifest {
    pub fn new(artifact_kind: ArtifactKind, document_sha256: String, document_bytes: u64) -> Self {
        Self {
            format: PACKAGE_MAGIC.to_owned(),
            package_version: PACKAGE_VERSION,
            document_version: DOCUMENT_VERSION,
            next_schema_version: NEXT_SCHEMA_VERSION,
            artifact_kind,
            document_path: DOCUMENT_PATH.to_owned(),
            document_sha256,
            document_bytes,
            assets: Vec::new(),
            previews: Vec::new(),
        }
    }

    pub fn validate(&self, limits: PackageLimits) -> Result<(), ManifestError> {
        if self.format != PACKAGE_MAGIC {
            return Err(ManifestError::InvalidFormat(self.format.clone()));
        }
        if self.package_version != PACKAGE_VERSION {
            return Err(ManifestError::UnsupportedPackageVersion {
                expected: PACKAGE_VERSION,
                actual: self.package_version,
            });
        }
        if self.document_version != DOCUMENT_VERSION {
            return Err(ManifestError::UnsupportedDocumentVersion {
                expected: DOCUMENT_VERSION,
                actual: self.document_version,
            });
        }
        if self.next_schema_version != NEXT_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedNextSchemaVersion {
                expected: NEXT_SCHEMA_VERSION,
                actual: self.next_schema_version,
            });
        }
        if self.document_path != DOCUMENT_PATH {
            return Err(ManifestError::InvalidDocumentPath(
                self.document_path.clone(),
            ));
        }
        validate_package_file_path(&self.document_path)?;
        validate_sha256(&self.document_sha256)?;
        if self.document_bytes > limits.max_document_bytes {
            return Err(ManifestError::DocumentTooLarge {
                actual: self.document_bytes,
                limit: limits.max_document_bytes,
            });
        }

        let entry_count = 2usize
            .checked_add(self.assets.len())
            .and_then(|value| value.checked_add(self.previews.len()))
            .ok_or(ManifestError::EntryCountOverflow)?;
        if entry_count > limits.max_entries {
            return Err(ManifestError::TooManyEntries {
                actual: entry_count,
                limit: limits.max_entries,
            });
        }

        let mut total_bytes = self.document_bytes;
        let mut asset_ids = BTreeSet::new();
        let mut content_hashes = BTreeSet::new();
        let mut paths = BTreeSet::from([MANIFEST_PATH.to_owned(), self.document_path.clone()]);

        for asset in &self.assets {
            if !asset_ids.insert(asset.id) {
                return Err(ManifestError::DuplicateAssetId(asset.id));
            }
            validate_sha256(&asset.content_sha256)?;
            validate_sha256(&asset.blob_sha256)?;
            if !content_hashes.insert(asset.content_sha256.clone()) {
                return Err(ManifestError::DuplicateAssetContentHash(
                    asset.content_sha256.clone(),
                ));
            }
            validate_package_file_path(&asset.path)?;
            if !asset.path.starts_with("assets/") {
                return Err(ManifestError::InvalidAssetPath(asset.path.clone()));
            }
            let file_name = asset
                .path
                .rsplit('/')
                .next()
                .ok_or_else(|| ManifestError::InvalidAssetPath(asset.path.clone()))?;
            let expected_prefix = format!("{}.", asset.content_sha256);
            if !file_name.starts_with(&expected_prefix) {
                return Err(ManifestError::AssetPathContentHashMismatch {
                    path: asset.path.clone(),
                    content_sha256: asset.content_sha256.clone(),
                });
            }
            if !paths.insert(asset.path.clone()) {
                return Err(ManifestError::DuplicatePath(asset.path.clone()));
            }
            if asset.bytes > limits.max_asset_bytes {
                return Err(ManifestError::AssetTooLarge {
                    path: asset.path.clone(),
                    actual: asset.bytes,
                    limit: limits.max_asset_bytes,
                });
            }
            total_bytes = total_bytes
                .checked_add(asset.bytes)
                .ok_or(ManifestError::TotalSizeOverflow)?;
        }

        for preview in &self.previews {
            validate_package_file_path(&preview.path)?;
            validate_sha256(&preview.sha256)?;
            if !preview.path.starts_with("previews/") {
                return Err(ManifestError::InvalidPreviewPath(preview.path.clone()));
            }
            if !paths.insert(preview.path.clone()) {
                return Err(ManifestError::DuplicatePath(preview.path.clone()));
            }
            total_bytes = total_bytes
                .checked_add(preview.bytes)
                .ok_or(ManifestError::TotalSizeOverflow)?;
        }

        if total_bytes > limits.max_total_uncompressed_bytes {
            return Err(ManifestError::PackageTooLarge {
                actual: total_bytes,
                limit: limits.max_total_uncompressed_bytes,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestAsset {
    pub id: AssetId,
    /// Stable semantic/content identity from `next-domain`.
    pub content_sha256: String,
    /// Integrity hash of the actual external package blob bytes.
    pub blob_sha256: String,
    pub media_type: String,
    pub bytes: u64,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestPreview {
    pub page_id: String,
    pub sha256: String,
    pub media_type: String,
    pub bytes: u64,
    pub path: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ManifestError {
    #[error("invalid DDNX format marker {0:?}")]
    InvalidFormat(String),
    #[error("unsupported DDNX package version {actual}; expected {expected}")]
    UnsupportedPackageVersion { expected: u32, actual: u32 },
    #[error("unsupported DDNX document projection version {actual}; expected {expected}")]
    UnsupportedDocumentVersion { expected: u32, actual: u32 },
    #[error("unsupported Next schema version {actual}; expected {expected}")]
    UnsupportedNextSchemaVersion { expected: u32, actual: u32 },
    #[error("document path must be \"document.json\", found {0:?}")]
    InvalidDocumentPath(String),
    #[error("invalid package path {0:?}")]
    InvalidPackagePath(String),
    #[error("invalid asset path {0:?}")]
    InvalidAssetPath(String),
    #[error("invalid preview path {0:?}")]
    InvalidPreviewPath(String),
    #[error("invalid SHA-256 {0:?}; expected 64 lowercase hexadecimal characters")]
    InvalidSha256(String),
    #[error("duplicate package path {0:?}")]
    DuplicatePath(String),
    #[error("duplicate asset id {0:?}")]
    DuplicateAssetId(AssetId),
    #[error("duplicate content-addressed asset hash {0}")]
    DuplicateAssetContentHash(String),
    #[error("asset path {path:?} does not start with content SHA-256 {content_sha256}")]
    AssetPathContentHashMismatch {
        path: String,
        content_sha256: String,
    },
    #[error("document payload is {actual} bytes; limit is {limit}")]
    DocumentTooLarge { actual: u64, limit: u64 },
    #[error("asset {path:?} is {actual} bytes; limit is {limit}")]
    AssetTooLarge {
        path: String,
        actual: u64,
        limit: u64,
    },
    #[error("package has {actual} entries; limit is {limit}")]
    TooManyEntries { actual: usize, limit: usize },
    #[error("package uncompressed payload is {actual} bytes; limit is {limit}")]
    PackageTooLarge { actual: u64, limit: u64 },
    #[error("package entry count overflow")]
    EntryCountOverflow,
    #[error("package uncompressed size overflow")]
    TotalSizeOverflow,
}

pub fn validate_package_file_path(path: &str) -> Result<(), ManifestError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.contains(':')
    {
        return Err(ManifestError::InvalidPackagePath(path.to_owned()));
    }

    for component in path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(ManifestError::InvalidPackagePath(path.to_owned()));
        }
    }

    Ok(())
}

pub fn validate_sha256(value: &str) -> Result<(), ManifestError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ManifestError::InvalidSha256(value.to_owned()));
    }
    Ok(())
}

pub fn asset_path(content_sha256: &str, extension: &str) -> Result<String, ManifestError> {
    validate_sha256(content_sha256)?;
    if extension.is_empty()
        || extension.len() > 12
        || !extension
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err(ManifestError::InvalidPackagePath(extension.to_owned()));
    }
    Ok(format!("assets/{content_sha256}.{extension}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    #[test]
    fn accepts_minimal_manifest() {
        let manifest = Manifest::new(ArtifactKind::Document, HASH_A.to_owned(), 42);
        assert_eq!(manifest.validate(PackageLimits::default()), Ok(()));
    }

    #[test]
    fn rejects_path_traversal_and_windows_paths() {
        for path in [
            "../document.json",
            "assets/../secret.bin",
            "/document.json",
            "C:/document.json",
            "assets\\file.bin",
            "assets//file.bin",
        ] {
            assert!(validate_package_file_path(path).is_err(), "{path}");
        }
    }

    #[test]
    fn requires_asset_file_name_to_match_content_hash() {
        let mut manifest = Manifest::new(ArtifactKind::Document, HASH_A.to_owned(), 42);
        manifest.assets.push(ManifestAsset {
            id: AssetId::new(),
            content_sha256: HASH_B.to_owned(),
            blob_sha256: HASH_C.to_owned(),
            media_type: "application/octet-stream".to_owned(),
            bytes: 4,
            path: format!("assets/{HASH_A}.bin"),
        });
        assert!(matches!(
            manifest.validate(PackageLimits::default()),
            Err(ManifestError::AssetPathContentHashMismatch { .. })
        ));
    }

    #[test]
    fn rejects_duplicate_content_hashes() {
        let mut manifest = Manifest::new(ArtifactKind::Document, HASH_A.to_owned(), 42);
        for index in 0..2 {
            manifest.assets.push(ManifestAsset {
                id: AssetId::new(),
                content_sha256: HASH_B.to_owned(),
                blob_sha256: if index == 0 {
                    HASH_A.to_owned()
                } else {
                    HASH_C.to_owned()
                },
                media_type: "application/octet-stream".to_owned(),
                bytes: 4,
                path: asset_path(HASH_B, "bin").unwrap(),
            });
        }
        assert!(matches!(
            manifest.validate(PackageLimits::default()),
            Err(ManifestError::DuplicateAssetContentHash(_))
        ));
    }

    #[test]
    fn rejects_total_uncompressed_size_over_limit() {
        let limits = PackageLimits {
            max_total_uncompressed_bytes: 10,
            ..PackageLimits::default()
        };
        let manifest = Manifest::new(ArtifactKind::Document, HASH_A.to_owned(), 11);
        assert!(matches!(
            manifest.validate(limits),
            Err(ManifestError::PackageTooLarge { .. })
        ));
    }
}
