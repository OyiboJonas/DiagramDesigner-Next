use std::f32::consts::FRAC_PI_2;

use legacy_ddd::{
    LegacyDecoded, LegacyFormat,
    object::{LegacyBaseObject, LegacyObject, LegacyPicturePayload, LegacyRect},
    template::LegacyTemplate,
};
use legacy_migrate::{MigrationOptions, migrate_decoded};
use next_domain::{Artifact, AssetPayload, ElementKind};

#[test]
fn converts_legacy_metafile_radians_to_next_degrees_without_touching_source_bytes() {
    let source_bytes = vec![0xd7, 0xcd, 0xc6, 0x9a, 0x01, 0x02];
    let decoded = LegacyDecoded::Ddt(LegacyTemplate {
        width: 252_000,
        height: 252_000,
        objects: vec![LegacyObject::Metafile {
            picture: LegacyPicturePayload {
                base: LegacyBaseObject {
                    name_raw: b"rotated metafile".to_vec(),
                    position: LegacyRect {
                        left: 0,
                        top: 0,
                        right: 25_200,
                        bottom: 12_600,
                    },
                    anchors: 0,
                },
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
