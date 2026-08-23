use std::collections::{HashMap, HashSet};

use next_domain::{
    Artifact, Asset, AssetId, AssetPayload, Document, DocumentDefaults, DocumentId, ElementStyle,
    ImportMetadata, Layer, NextArtifact, Page, Scene, Size, TemplateId, TemplatePalette,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ArtifactKind, DOCUMENT_VERSION, PackageAssetDescriptor, PackageAssetKind, ReadPackage,
};

#[derive(Debug, Error)]
pub enum HydrationError {
    #[error("DDNX document JSON failed to parse: {0}")]
    DocumentJson(#[from] serde_json::Error),
    #[error("DDNX document projection version {actual} does not match manifest version {expected}")]
    DocumentVersion { expected: u32, actual: u32 },
    #[error(
        "Next schema version {actual} in document JSON does not match manifest version {expected}"
    )]
    NextSchemaVersion { expected: u32, actual: u32 },
    #[error("DDNX artifact kind in document JSON does not match manifest kind")]
    ArtifactKindMismatch,
    #[error(
        "document JSON describes {descriptors} assets but package contains {blobs} asset blobs"
    )]
    AssetCountMismatch { descriptors: usize, blobs: usize },
    #[error("document JSON contains duplicate asset descriptor id {0:?}")]
    DuplicateAssetDescriptor(AssetId),
    #[error("document JSON references asset {0:?} that has no verified package blob")]
    MissingAssetBlob(AssetId),
    #[error("verified package blob {0:?} is not referenced by document JSON")]
    UnexpectedAssetBlob(AssetId),
    #[error("asset descriptor metadata does not match verified blob for asset {0:?}")]
    AssetMetadataMismatch(AssetId),
    #[error("asset {asset_id:?} blob layout is invalid: {reason}")]
    InvalidAssetLayout { asset_id: AssetId, reason: String },
    #[error(
        "asset {asset_id:?} semantic content SHA-256 mismatch: expected {expected}, found {actual}"
    )]
    AssetContentHashMismatch {
        asset_id: AssetId,
        expected: String,
        actual: String,
    },
    #[error("hydrated Next artifact is structurally invalid ({issues} validation issue(s))")]
    InvalidArtifact { issues: usize },
}

#[derive(Debug, Deserialize)]
struct PackageDocumentRoot {
    document_version: u32,
    next_schema_version: u32,
    artifact: PackageArtifact,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum PackageArtifact {
    Document(PackageDocument),
    TemplatePalette(PackageTemplatePalette),
}

#[derive(Debug, Deserialize)]
struct PackageDocument {
    id: DocumentId,
    name: String,
    defaults: DocumentDefaults,
    #[serde(default)]
    master_layers: Vec<Layer>,
    pages: Vec<Page>,
    styles: Vec<ElementStyle>,
    assets: Vec<PackageAssetDescriptor>,
    import: Option<ImportMetadata>,
}

#[derive(Debug, Deserialize)]
struct PackageTemplatePalette {
    id: TemplateId,
    name: String,
    size_mm: Size,
    scene: Scene,
    styles: Vec<ElementStyle>,
    assets: Vec<PackageAssetDescriptor>,
    import: Option<ImportMetadata>,
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

fn semantic_asset_hash(asset: &AssetPayload) -> String {
    if let AssetPayload::Binary { bytes } = asset {
        return sha256_hex(bytes);
    }

    let mut hasher = Sha256::new();
    if let AssetPayload::Raster {
        width,
        height,
        bits_per_pixel,
        palette,
        pixels,
        alpha,
        alpha_value,
    } = asset
    {
        hasher.update(width.to_le_bytes());
        hasher.update(height.to_le_bytes());
        hasher.update([*bits_per_pixel, *alpha_value]);
        if let Some(palette) = palette {
            hasher.update(palette);
        }
        hasher.update(pixels);
        if let Some(alpha) = alpha {
            hasher.update(alpha);
        }
    }

    let digest = hasher.finalize();
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn checked_usize(value: u64, asset_id: AssetId) -> Result<usize, HydrationError> {
    usize::try_from(value).map_err(|_| HydrationError::InvalidAssetLayout {
        asset_id,
        reason: format!("length {value} does not fit into usize"),
    })
}

fn hydrate_asset(
    descriptor: PackageAssetDescriptor,
    blob: crate::PreparedAssetBlob,
) -> Result<Asset, HydrationError> {
    if descriptor.id != blob.id
        || descriptor.content_sha256 != blob.content_sha256
        || descriptor.media_type != blob.media_type
    {
        return Err(HydrationError::AssetMetadataMismatch(descriptor.id));
    }

    let payload = match descriptor.kind {
        PackageAssetKind::Binary => AssetPayload::Binary { bytes: blob.bytes },
        PackageAssetKind::Raster {
            width,
            height,
            bits_per_pixel,
            alpha_value,
            palette_present,
            palette_bytes,
            pixel_bytes,
            alpha_present,
            alpha_bytes,
        } => {
            if !palette_present && palette_bytes != 0 {
                return Err(HydrationError::InvalidAssetLayout {
                    asset_id: descriptor.id,
                    reason: "palette is marked absent but palette_bytes is non-zero".to_owned(),
                });
            }
            if !alpha_present && alpha_bytes != 0 {
                return Err(HydrationError::InvalidAssetLayout {
                    asset_id: descriptor.id,
                    reason: "alpha is marked absent but alpha_bytes is non-zero".to_owned(),
                });
            }

            let palette_len = checked_usize(palette_bytes, descriptor.id)?;
            let pixel_len = checked_usize(pixel_bytes, descriptor.id)?;
            let alpha_len = checked_usize(alpha_bytes, descriptor.id)?;
            let expected = palette_len
                .checked_add(pixel_len)
                .and_then(|value| value.checked_add(alpha_len))
                .ok_or_else(|| HydrationError::InvalidAssetLayout {
                    asset_id: descriptor.id,
                    reason: "component lengths overflow usize".to_owned(),
                })?;
            if expected != blob.bytes.len() {
                return Err(HydrationError::InvalidAssetLayout {
                    asset_id: descriptor.id,
                    reason: format!(
                        "descriptor expects {expected} bytes, blob contains {}",
                        blob.bytes.len()
                    ),
                });
            }

            let palette_end = palette_len;
            let pixels_end = palette_end + pixel_len;
            let palette = palette_present.then(|| blob.bytes[..palette_end].to_vec());
            let pixels = blob.bytes[palette_end..pixels_end].to_vec();
            let alpha = alpha_present.then(|| blob.bytes[pixels_end..].to_vec());
            AssetPayload::Raster {
                width,
                height,
                bits_per_pixel,
                palette,
                pixels,
                alpha,
                alpha_value,
            }
        }
    };

    let actual_hash = semantic_asset_hash(&payload);
    if actual_hash != descriptor.content_sha256 {
        return Err(HydrationError::AssetContentHashMismatch {
            asset_id: descriptor.id,
            expected: descriptor.content_sha256,
            actual: actual_hash,
        });
    }

    Ok(Asset {
        id: descriptor.id,
        sha256: actual_hash,
        media_type: descriptor.media_type,
        payload,
    })
}

fn hydrate_assets(
    descriptors: Vec<PackageAssetDescriptor>,
    blobs: Vec<crate::PreparedAssetBlob>,
) -> Result<Vec<Asset>, HydrationError> {
    if descriptors.len() != blobs.len() {
        return Err(HydrationError::AssetCountMismatch {
            descriptors: descriptors.len(),
            blobs: blobs.len(),
        });
    }

    let mut blobs_by_id: HashMap<AssetId, _> =
        blobs.into_iter().map(|blob| (blob.id, blob)).collect();
    if blobs_by_id.len() != descriptors.len() {
        return Err(HydrationError::AssetCountMismatch {
            descriptors: descriptors.len(),
            blobs: blobs_by_id.len(),
        });
    }

    let mut seen = HashSet::new();
    let mut assets = Vec::with_capacity(descriptors.len());
    for descriptor in descriptors {
        if !seen.insert(descriptor.id) {
            return Err(HydrationError::DuplicateAssetDescriptor(descriptor.id));
        }
        let blob = blobs_by_id
            .remove(&descriptor.id)
            .ok_or(HydrationError::MissingAssetBlob(descriptor.id))?;
        assets.push(hydrate_asset(descriptor, blob)?);
    }
    if let Some(asset_id) = blobs_by_id.keys().next().copied() {
        return Err(HydrationError::UnexpectedAssetBlob(asset_id));
    }
    Ok(assets)
}

impl ReadPackage {
    /// Hydrate an already ZIP- and hash-verified package into the renderer-independent
    /// Next domain. Asset bytes are reconstructed from the external package blobs and
    /// their semantic content identity is verified before the artifact is returned.
    pub fn into_artifact(self) -> Result<NextArtifact, HydrationError> {
        let root: PackageDocumentRoot = serde_json::from_slice(&self.document_json)?;
        if root.document_version != self.manifest.document_version
            || root.document_version != DOCUMENT_VERSION
        {
            return Err(HydrationError::DocumentVersion {
                expected: self.manifest.document_version,
                actual: root.document_version,
            });
        }
        if root.next_schema_version != self.manifest.next_schema_version {
            return Err(HydrationError::NextSchemaVersion {
                expected: self.manifest.next_schema_version,
                actual: root.next_schema_version,
            });
        }

        let artifact = match root.artifact {
            PackageArtifact::Document(document) => {
                if self.manifest.artifact_kind != ArtifactKind::Document {
                    return Err(HydrationError::ArtifactKindMismatch);
                }
                let assets = hydrate_assets(document.assets, self.asset_blobs)?;
                NextArtifact {
                    schema_version: root.next_schema_version,
                    artifact: Artifact::Document(Document {
                        id: document.id,
                        name: document.name,
                        defaults: document.defaults,
                        master_layers: document.master_layers,
                        pages: document.pages,
                        styles: document.styles,
                        assets,
                        import: document.import,
                    }),
                }
            }
            PackageArtifact::TemplatePalette(template) => {
                if self.manifest.artifact_kind != ArtifactKind::TemplatePalette {
                    return Err(HydrationError::ArtifactKindMismatch);
                }
                let assets = hydrate_assets(template.assets, self.asset_blobs)?;
                NextArtifact {
                    schema_version: root.next_schema_version,
                    artifact: Artifact::TemplatePalette(TemplatePalette {
                        id: template.id,
                        name: template.name,
                        size_mm: template.size_mm,
                        scene: template.scene,
                        styles: template.styles,
                        assets,
                        import: template.import,
                    }),
                }
            }
        };

        let validation = artifact.validate();
        if !validation.is_valid() {
            return Err(HydrationError::InvalidArtifact {
                issues: validation.issues.len(),
            });
        }
        Ok(artifact)
    }
}

#[cfg(test)]
mod tests {
    use next_domain::{Asset, AssetPayload, Scene, TemplatePalette};

    use super::*;
    use crate::{PackageLimits, prepare_package, read_package, write_package_to_vec};

    fn binary_fixture() -> NextArtifact {
        let bytes = vec![1, 2, 3, 4];
        let hash = sha256_hex(&bytes);
        NextArtifact::template_palette(TemplatePalette {
            id: TemplateId::new(),
            name: "Hydration test".to_owned(),
            size_mm: Size {
                width: 10.0,
                height: 20.0,
            },
            scene: Scene::default(),
            styles: Vec::new(),
            assets: vec![Asset {
                id: AssetId::new(),
                sha256: hash,
                media_type: "application/octet-stream".to_owned(),
                payload: AssetPayload::Binary { bytes },
            }],
            import: None,
        })
    }

    fn raster_fixture(palette: Option<Vec<u8>>, alpha: Option<Vec<u8>>) -> NextArtifact {
        let payload = AssetPayload::Raster {
            width: 2,
            height: 2,
            bits_per_pixel: 8,
            palette,
            pixels: vec![3, 4, 5, 6],
            alpha,
            alpha_value: 255,
        };
        let hash = semantic_asset_hash(&payload);
        NextArtifact::template_palette(TemplatePalette {
            id: TemplateId::new(),
            name: "Raster hydration test".to_owned(),
            size_mm: Size {
                width: 10.0,
                height: 20.0,
            },
            scene: Scene::default(),
            styles: Vec::new(),
            assets: vec![Asset {
                id: AssetId::new(),
                sha256: hash,
                media_type: "application/vnd.diagramdesigner-next.raster".to_owned(),
                payload,
            }],
            import: None,
        })
    }

    fn round_trip(artifact: NextArtifact) {
        let limits = PackageLimits::default();
        let prepared = prepare_package(&artifact, limits).unwrap();
        let bytes = write_package_to_vec(&prepared, limits).unwrap();
        let hydrated = read_package(&bytes, limits)
            .unwrap()
            .into_artifact()
            .unwrap();
        assert_eq!(hydrated, artifact);
    }

    #[test]
    fn hydrates_binary_asset_round_trip() {
        round_trip(binary_fixture());
    }

    #[test]
    fn hydrates_raster_asset_round_trip() {
        round_trip(raster_fixture(Some(vec![0, 1, 2]), Some(vec![7, 8, 9, 10])));
    }

    #[test]
    fn preserves_present_but_empty_raster_components() {
        round_trip(raster_fixture(Some(Vec::new()), Some(Vec::new())));
    }

    #[test]
    fn distinguishes_absent_raster_components() {
        round_trip(raster_fixture(None, None));
    }

    #[test]
    fn rejects_false_semantic_asset_hash() {
        let mut artifact = binary_fixture();
        let Artifact::TemplatePalette(template) = &mut artifact.artifact else {
            unreachable!();
        };
        template.assets[0].sha256 =
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
        let limits = PackageLimits::default();
        let prepared = prepare_package(&artifact, limits).unwrap();
        let bytes = write_package_to_vec(&prepared, limits).unwrap();
        let error = read_package(&bytes, limits)
            .unwrap()
            .into_artifact()
            .unwrap_err();
        assert!(matches!(
            error,
            HydrationError::AssetContentHashMismatch { .. }
        ));
    }
}
