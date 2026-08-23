use next_domain::{
    Artifact, Asset, AssetId, AssetPayload, DocumentDefaults, DocumentId, ElementStyle,
    ImportMetadata, Layer, NextArtifact, Page, SCHEMA_VERSION, Scene, Size, TemplateId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ArtifactKind, DOCUMENT_VERSION, Manifest, ManifestAsset, ManifestError, PackageLimits,
    asset_path, validate_sha256,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageAssetDescriptor {
    pub id: AssetId,
    /// Stable semantic/content identity owned by `next-domain`.
    pub content_sha256: String,
    pub media_type: String,
    pub kind: PackageAssetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PackageAssetKind {
    Raster {
        width: i32,
        height: i32,
        bits_per_pixel: u8,
        alpha_value: u8,
        palette_present: bool,
        palette_bytes: u64,
        pixel_bytes: u64,
        alpha_present: bool,
        alpha_bytes: u64,
    },
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedAssetBlob {
    pub id: AssetId,
    pub content_sha256: String,
    pub blob_sha256: String,
    pub path: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedPackage {
    pub manifest: Manifest,
    pub manifest_json: Vec<u8>,
    pub document_json: Vec<u8>,
    pub asset_blobs: Vec<PreparedAssetBlob>,
}

#[derive(Debug, Error)]
pub enum PreparationError {
    #[error("Next artifact is structurally invalid ({issues} validation issue(s))")]
    InvalidArtifact { issues: usize },
    #[error("asset {asset_id:?} payload length overflow")]
    AssetLengthOverflow { asset_id: AssetId },
    #[error("asset {asset_id:?} is {actual} bytes; limit is {limit}")]
    AssetTooLarge {
        asset_id: AssetId,
        actual: u64,
        limit: u64,
    },
    #[error("serialized manifest is {actual} bytes; limit is {limit}")]
    ManifestTooLarge { actual: u64, limit: u64 },
    #[error("prepared package is {actual} uncompressed bytes; limit is {limit}")]
    PackageTooLarge { actual: u64, limit: u64 },
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("DDNX JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Serialize)]
struct PackageDocumentRoot<'a> {
    document_version: u32,
    next_schema_version: u32,
    artifact: PackageArtifact<'a>,
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum PackageArtifact<'a> {
    Document(PackageDocument<'a>),
    TemplatePalette(PackageTemplatePalette<'a>),
}

#[derive(Serialize)]
struct PackageDocument<'a> {
    id: DocumentId,
    name: &'a str,
    defaults: &'a DocumentDefaults,
    master_layers: &'a [Layer],
    pages: &'a [Page],
    styles: &'a [ElementStyle],
    assets: &'a [PackageAssetDescriptor],
    #[serde(skip_serializing_if = "Option::is_none")]
    import: Option<&'a ImportMetadata>,
}

#[derive(Serialize)]
struct PackageTemplatePalette<'a> {
    id: TemplateId,
    name: &'a str,
    size_mm: Size,
    scene: &'a Scene,
    styles: &'a [ElementStyle],
    assets: &'a [PackageAssetDescriptor],
    #[serde(skip_serializing_if = "Option::is_none")]
    import: Option<&'a ImportMetadata>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn usize_to_u64(value: usize, asset_id: AssetId) -> Result<u64, PreparationError> {
    u64::try_from(value).map_err(|_| PreparationError::AssetLengthOverflow { asset_id })
}

fn prepare_asset(
    asset: &Asset,
    limits: PackageLimits,
) -> Result<(PackageAssetDescriptor, PreparedAssetBlob), PreparationError> {
    validate_sha256(&asset.sha256)?;

    let (kind, blob_bytes) = match &asset.payload {
        AssetPayload::Raster {
            width,
            height,
            bits_per_pixel,
            palette,
            pixels,
            alpha,
            alpha_value,
        } => {
            let palette_len = palette.as_ref().map_or(0, Vec::len);
            let pixel_len = pixels.len();
            let alpha_len = alpha.as_ref().map_or(0, Vec::len);
            let total_len = palette_len
                .checked_add(pixel_len)
                .and_then(|value| value.checked_add(alpha_len))
                .ok_or(PreparationError::AssetLengthOverflow { asset_id: asset.id })?;
            let total_bytes = usize_to_u64(total_len, asset.id)?;
            if total_bytes > limits.max_asset_bytes {
                return Err(PreparationError::AssetTooLarge {
                    asset_id: asset.id,
                    actual: total_bytes,
                    limit: limits.max_asset_bytes,
                });
            }

            let mut blob = Vec::with_capacity(total_len);
            if let Some(palette) = palette {
                blob.extend_from_slice(palette);
            }
            blob.extend_from_slice(pixels);
            if let Some(alpha) = alpha {
                blob.extend_from_slice(alpha);
            }

            (
                PackageAssetKind::Raster {
                    width: *width,
                    height: *height,
                    bits_per_pixel: *bits_per_pixel,
                    alpha_value: *alpha_value,
                    palette_present: palette.is_some(),
                    palette_bytes: usize_to_u64(palette_len, asset.id)?,
                    pixel_bytes: usize_to_u64(pixel_len, asset.id)?,
                    alpha_present: alpha.is_some(),
                    alpha_bytes: usize_to_u64(alpha_len, asset.id)?,
                },
                blob,
            )
        }
        AssetPayload::Binary { bytes } => {
            let byte_len = usize_to_u64(bytes.len(), asset.id)?;
            if byte_len > limits.max_asset_bytes {
                return Err(PreparationError::AssetTooLarge {
                    asset_id: asset.id,
                    actual: byte_len,
                    limit: limits.max_asset_bytes,
                });
            }
            (PackageAssetKind::Binary, bytes.clone())
        }
    };

    let blob_sha256 = sha256_hex(&blob_bytes);
    let path = asset_path(&asset.sha256, "bin")?;
    let descriptor = PackageAssetDescriptor {
        id: asset.id,
        content_sha256: asset.sha256.clone(),
        media_type: asset.media_type.clone(),
        kind,
    };
    let blob = PreparedAssetBlob {
        id: asset.id,
        content_sha256: asset.sha256.clone(),
        blob_sha256,
        path,
        media_type: asset.media_type.clone(),
        bytes: blob_bytes,
    };
    Ok((descriptor, blob))
}

fn package_artifact<'a>(
    artifact: &'a NextArtifact,
    asset_descriptors: &'a [PackageAssetDescriptor],
) -> (ArtifactKind, PackageArtifact<'a>) {
    match &artifact.artifact {
        Artifact::Document(document) => (
            ArtifactKind::Document,
            PackageArtifact::Document(PackageDocument {
                id: document.id,
                name: &document.name,
                defaults: &document.defaults,
                master_layers: &document.master_layers,
                pages: &document.pages,
                styles: &document.styles,
                assets: asset_descriptors,
                import: document.import.as_ref(),
            }),
        ),
        Artifact::TemplatePalette(template) => (
            ArtifactKind::TemplatePalette,
            PackageArtifact::TemplatePalette(PackageTemplatePalette {
                id: template.id,
                name: &template.name,
                size_mm: template.size_mm,
                scene: &template.scene,
                styles: &template.styles,
                assets: asset_descriptors,
                import: template.import.as_ref(),
            }),
        ),
    }
}

fn artifact_assets(artifact: &NextArtifact) -> &[Asset] {
    match &artifact.artifact {
        Artifact::Document(document) => &document.assets,
        Artifact::TemplatePalette(template) => &template.assets,
    }
}

/// Prepare a validated Next artifact for DDNX storage without introducing ZIP
/// concerns or native file paths into `next-domain`.
///
/// Binary payloads are externalized exactly once. `document_json` contains only
/// asset descriptors and metadata; `asset_blobs` contains the bytes. The manifest
/// binds the semantic content hash to a separate blob-integrity hash.
pub fn prepare_package(
    artifact: &NextArtifact,
    limits: PackageLimits,
) -> Result<PreparedPackage, PreparationError> {
    let validation = artifact.validate();
    if !validation.is_valid() {
        return Err(PreparationError::InvalidArtifact {
            issues: validation.issues.len(),
        });
    }

    // Preserve domain asset order in document.json so a load/save round-trip is
    // structurally exact. Physical ZIP entries are sorted independently by path.
    let prepared_assets = artifact_assets(artifact)
        .iter()
        .map(|asset| prepare_asset(asset, limits))
        .collect::<Result<Vec<_>, _>>()?;

    let asset_descriptors: Vec<_> = prepared_assets
        .iter()
        .map(|(descriptor, _)| descriptor.clone())
        .collect();
    let asset_blobs: Vec<_> = prepared_assets.into_iter().map(|(_, blob)| blob).collect();

    let (artifact_kind, package_artifact) = package_artifact(artifact, &asset_descriptors);
    let document_root = PackageDocumentRoot {
        document_version: DOCUMENT_VERSION,
        next_schema_version: SCHEMA_VERSION,
        artifact: package_artifact,
    };
    let document_json = serde_json::to_vec(&document_root)?;
    let document_bytes =
        u64::try_from(document_json.len()).map_err(|_| PreparationError::PackageTooLarge {
            actual: u64::MAX,
            limit: limits.max_total_uncompressed_bytes,
        })?;
    let document_sha256 = sha256_hex(&document_json);

    let mut manifest = Manifest::new(artifact_kind, document_sha256, document_bytes);
    manifest.assets = asset_blobs
        .iter()
        .map(|blob| ManifestAsset {
            id: blob.id,
            content_sha256: blob.content_sha256.clone(),
            blob_sha256: blob.blob_sha256.clone(),
            media_type: blob.media_type.clone(),
            bytes: blob.bytes.len() as u64,
            path: blob.path.clone(),
        })
        .collect();
    manifest.validate(limits)?;

    let manifest_json = serde_json::to_vec(&manifest)?;
    let manifest_bytes =
        u64::try_from(manifest_json.len()).map_err(|_| PreparationError::ManifestTooLarge {
            actual: u64::MAX,
            limit: limits.max_manifest_bytes,
        })?;
    if manifest_bytes > limits.max_manifest_bytes {
        return Err(PreparationError::ManifestTooLarge {
            actual: manifest_bytes,
            limit: limits.max_manifest_bytes,
        });
    }

    let total_uncompressed = manifest_bytes
        .checked_add(document_bytes)
        .and_then(|value| {
            asset_blobs.iter().try_fold(value, |total, blob| {
                total.checked_add(blob.bytes.len() as u64)
            })
        })
        .ok_or(PreparationError::PackageTooLarge {
            actual: u64::MAX,
            limit: limits.max_total_uncompressed_bytes,
        })?;
    if total_uncompressed > limits.max_total_uncompressed_bytes {
        return Err(PreparationError::PackageTooLarge {
            actual: total_uncompressed,
            limit: limits.max_total_uncompressed_bytes,
        });
    }

    Ok(PreparedPackage {
        manifest,
        manifest_json,
        document_json,
        asset_blobs,
    })
}

#[cfg(test)]
mod tests {
    use next_domain::{
        Asset, AssetPayload, NextArtifact, Scene, Size, TemplateId, TemplatePalette,
    };

    use super::*;

    const CONTENT_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn artifact_with_binary_asset() -> NextArtifact {
        NextArtifact::template_palette(TemplatePalette {
            id: TemplateId::new(),
            name: "Palette".to_owned(),
            size_mm: Size {
                width: 10.0,
                height: 20.0,
            },
            scene: Scene::default(),
            styles: Vec::new(),
            assets: vec![Asset {
                id: AssetId::new(),
                sha256: CONTENT_HASH.to_owned(),
                media_type: "application/octet-stream".to_owned(),
                payload: AssetPayload::Binary {
                    bytes: vec![1, 2, 3, 4],
                },
            }],
            import: None,
        })
    }

    #[test]
    fn externalizes_binary_payload_from_document_json() {
        let prepared = prepare_package(&artifact_with_binary_asset(), PackageLimits::default())
            .expect("package preparation");
        assert_eq!(prepared.asset_blobs.len(), 1);
        assert_eq!(prepared.asset_blobs[0].bytes, vec![1, 2, 3, 4]);
        let document = String::from_utf8(prepared.document_json).unwrap();
        assert!(!document.contains("\"bytes\":[1,2,3,4]"));
        assert!(document.contains(CONTENT_HASH));
        assert_eq!(prepared.manifest.assets.len(), 1);
        assert_ne!(
            prepared.manifest.assets[0].content_sha256,
            prepared.manifest.assets[0].blob_sha256
        );
    }

    #[test]
    fn preparation_is_deterministic_for_same_artifact() {
        let artifact = artifact_with_binary_asset();
        let first = prepare_package(&artifact, PackageLimits::default()).unwrap();
        let second = prepare_package(&artifact, PackageLimits::default()).unwrap();
        assert_eq!(first.manifest_json, second.manifest_json);
        assert_eq!(first.document_json, second.document_json);
        assert_eq!(first.asset_blobs, second.asset_blobs);
    }
}
