//! Phase-2 public migration facade.
//!
//! The stable legacy importer remains the parsing/mapping core. This facade owns
//! compatibility corrections that translate legacy representation units into the
//! renderer-independent Next domain without changing the preserved source bytes.

#[path = "lib.rs"]
mod existing;

pub use existing::{MigrationError, MigrationOptions};

use legacy_ddd::{LegacyDecoded, LegacyFormat};
use next_domain::{Artifact, ElementKind, NextArtifact, Scene};

pub fn migrate_bytes(
    bytes: &[u8],
    max_inflated_bytes: usize,
    options: MigrationOptions,
) -> Result<NextArtifact, MigrationError> {
    let mut artifact = existing::migrate_bytes(bytes, max_inflated_bytes, options)?;
    normalize_legacy_rotations(&mut artifact);
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
    normalize_legacy_rotations(&mut artifact);
    Ok(artifact)
}

fn normalize_legacy_rotations(artifact: &mut NextArtifact) {
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
        let raw_key = match &element.kind {
            ElementKind::Text => Some("text_angle_bits"),
            ElementKind::Metafile { .. } => Some("metafile_angle_bits"),
            _ => None,
        };
        let Some(raw_key) = raw_key else {
            continue;
        };

        // Public DiagramDesigner `TTextObject` and `TMetafileObject` both store a
        // Delphi `Single` angle in radians. Text drawing explicitly converts with
        // `Angle * 180 / Pi`; metafile drawing feeds the same unit to Sin/Cos and
        // RotatePoint. Next deliberately names the destination `rotation_deg`, so
        // the representation conversion belongs at this legacy import boundary.
        let legacy_radians = element.rotation_deg as f32;
        if let Some(import) = &mut element.import {
            import
                .raw_values
                .insert(raw_key.to_owned(), legacy_radians.to_bits() as i64);
        }
        element.rotation_deg = (legacy_radians as f64).to_degrees();
    }
}
