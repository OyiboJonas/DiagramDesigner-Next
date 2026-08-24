//! Phase-2 public migration facade.
//!
//! The stable legacy importer remains the parsing/mapping core. This facade owns
//! compatibility corrections that translate legacy representation units into the
//! renderer-independent Next domain without changing the preserved source bytes.

#[path = "lib.rs"]
mod existing;

pub use existing::{MigrationError, MigrationOptions};

use legacy_ddd::{LegacyDecoded, LegacyFormat, encoding::LegacyEncoding};
use next_domain::{Artifact, ElementKind, NextArtifact, Scene};

pub fn migrate_bytes(
    bytes: &[u8],
    max_inflated_bytes: usize,
    options: MigrationOptions,
) -> Result<NextArtifact, MigrationError> {
    let mut artifact = existing::migrate_bytes(bytes, max_inflated_bytes, options)?;
    normalize_legacy_metafile_rotations(&mut artifact);
    Ok(artifact)
}

pub fn migrate_decoded(
    decoded: &LegacyDecoded,
    source_format: LegacyFormat,
    source_version: u16,
    source_sha256: &str,
    options: MigrationOptions,
) -> Result<NextArtifact, MigrationError> {
    let mut artifact = existing::migrate_decoded(
        decoded,
        source_format,
        source_version,
        source_sha256,
        options,
    )?;
    normalize_legacy_metafile_rotations(&mut artifact);
    Ok(artifact)
}

fn normalize_legacy_metafile_rotations(artifact: &mut NextArtifact) {
    match &mut artifact.artifact {
        Artifact::Document(document) => {
            for layer in &mut document.master_layers {
                normalize_scene(&mut layer.scene);
            }
            for page in &mut document.pages {
                for layer in &mut page.layers {
                    normalize_scene(&mut layer.scene);
                }
            }
        }
        Artifact::TemplatePalette(template) => normalize_scene(&mut template.scene),
    }
}

fn normalize_scene(scene: &mut Scene) {
    for element in &mut scene.elements {
        if !matches!(element.kind, ElementKind::Metafile { .. }) {
            continue;
        }

        // `TMetafileObject` stores a Delphi `Single` angle and feeds it directly
        // to Sin/Cos and RotatePoint. That serialized value is therefore radians.
        // The Next domain deliberately names the destination field `rotation_deg`,
        // so conversion belongs at the legacy import boundary.
        let legacy_radians = element.rotation_deg as f32;
        if let Some(import) = &mut element.import {
            import.raw_values.insert(
                "metafile_angle_bits".to_owned(),
                legacy_radians.to_bits() as i64,
            );
        }
        element.rotation_deg = (legacy_radians as f64).to_degrees();
    }
}

#[allow(dead_code)]
fn _encoding_type_is_part_of_public_signature(_: LegacyEncoding) {}
