use std::f32::consts::FRAC_PI_2;

use legacy_ddd::{
    LegacyDecoded, LegacyFormat,
    object::{LegacyBaseObject, LegacyObject, LegacyPicturePayload, LegacyRect, LegacyTextPayload},
    template::LegacyTemplate,
};
use legacy_migrate::{MigrationOptions, migrate_decoded};
use next_domain::{Artifact, AssetPayload, ElementKind};

fn base(name: &[u8]) -> LegacyBaseObject {
    LegacyBaseObject {
        name_raw: name.to_vec(),
        position: LegacyRect {
            left: 0,
            top: 0,
            right: 25_200,
            bottom: 12_600,
        },
        anchors: 0,
    }
}

#[test]
fn converts_legacy_metafile_radians_to_next_degrees_without_touching_source_bytes() {
    let source_bytes = vec![0xd7, 0xcd, 0xc6, 0x9a, 0x01, 0x02];
    let decoded = LegacyDecoded::Ddt(LegacyTemplate {
        width: 252_000,
        height: 252_000,
        objects: vec![LegacyObject::Metafile {
            picture: LegacyPicturePayload {
                base: base(b"rotated metafile"),
                links: Vec::new(),
            },
            metafile_raw: source_bytes.clone(),
            angle: FRAC_PI_2,
        }],
        trailing_bytes: 0,
    });

    let artifact = migrate_decoded(
        &decoded,
        LegacyFormat::Ddt,
        28,
        "public-synthetic-metafile-angle",
        MigrationOptions::default(),
    )
    .unwrap();

    let Artifact::TemplatePalette(template) = artifact.artifact else {
        panic!("expected template palette");
    };
    let element = &template.scene.elements[0];
    assert!(matches!(&element.kind, ElementKind::Metafile { .. }));
    assert!((element.rotation_deg - 90.0).abs() < 1.0e-4);
    assert_eq!(
        element
            .import
            .as_ref()
            .unwrap()
            .raw_values
            .get("metafile_angle_bits"),
        Some(&(FRAC_PI_2.to_bits() as i64))
    );

    let ElementKind::Metafile { asset_id } = &element.kind else {
        unreachable!();
    };
    let asset = template
        .assets
        .iter()
        .find(|asset| asset.id == *asset_id)
        .unwrap();
    assert_eq!(
        asset.payload,
        AssetPayload::Binary {
            bytes: source_bytes,
        }
    );
}

#[test]
fn converts_legacy_text_radians_to_next_degrees_and_preserves_raw_single_bits() {
    let decoded = LegacyDecoded::Ddt(LegacyTemplate {
        width: 252_000,
        height: 252_000,
        objects: vec![LegacyObject::Text {
            payload: LegacyTextPayload {
                base: base(b"rotated text"),
                text_raw: b"Angle".to_vec(),
                text_x_align: 0,
                text_y_align: 0,
                text_color: 0,
                margin: 0,
                angle: FRAC_PI_2,
            },
        }],
        trailing_bytes: 0,
    });

    let artifact = migrate_decoded(
        &decoded,
        LegacyFormat::Ddt,
        28,
        "public-synthetic-text-angle",
        MigrationOptions::default(),
    )
    .unwrap();

    let Artifact::TemplatePalette(template) = artifact.artifact else {
        panic!("expected template palette");
    };
    let element = &template.scene.elements[0];
    assert!(matches!(&element.kind, ElementKind::Text));
    assert!((element.rotation_deg - 90.0).abs() < 1.0e-4);
    assert_eq!(
        element
            .import
            .as_ref()
            .unwrap()
            .raw_values
            .get("text_angle_bits"),
        Some(&(FRAC_PI_2.to_bits() as i64))
    );
}
