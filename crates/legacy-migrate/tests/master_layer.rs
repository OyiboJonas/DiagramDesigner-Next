use legacy_ddd::{
    LegacyDecoded, LegacyFormat,
    container::{LegacyContainer, LegacyContainerDefaults, LegacyLayer},
    object::{LegacyBaseObject, LegacyObject, LegacyRect},
};
use legacy_migrate::{MigrationOptions, migrate_decoded};
use next_domain::{Artifact, ElementKind};

fn base_object(name: &[u8]) -> LegacyBaseObject {
    LegacyBaseObject {
        name_raw: name.to_vec(),
        position: LegacyRect {
            left: 0,
            top: 0,
            right: 2520,
            bottom: 2520,
        },
        anchors: 0,
    }
}

#[test]
fn non_empty_legacy_stencil_maps_to_document_master_layer() {
    let decoded = LegacyDecoded::Ddd(LegacyContainer {
        defaults: LegacyContainerDefaults {
            default_font_name_raw: b"Arial".to_vec(),
            default_font_size: 10,
            default_font_style: 0,
            default_font_charset: 1,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: 1,
        },
        pages: Vec::new(),
        stencil: Some(LegacyLayer {
            draw_color: -1,
            objects: vec![LegacyObject::Group {
                base: base_object(b"Shared group"),
                links: Vec::new(),
                children: Vec::new(),
            }],
        }),
        trailing_bytes: 0,
    });

    let artifact = migrate_decoded(
        &decoded,
        LegacyFormat::Ddd,
        28,
        "0123456789abcdef",
        MigrationOptions::default(),
    )
    .unwrap();

    let Artifact::Document(document) = artifact.artifact else {
        panic!("expected document");
    };
    assert_eq!(document.master_layers.len(), 1);
    let master = &document.master_layers[0];
    assert_eq!(master.name, "Shared background");
    assert_eq!(master.scene.roots.len(), 1);
    assert_eq!(master.scene.elements.len(), 1);
    assert!(matches!(
        master.scene.elements[0].kind,
        ElementKind::Group { .. }
    ));
    assert_eq!(
        master.scene.elements[0]
            .import
            .as_ref()
            .expect("import metadata")
            .source_path,
        "stencil/object/0"
    );
    assert!(document.import.as_ref().is_some_and(|metadata| {
        metadata
            .diagnostics
            .iter()
            .any(|entry| entry.contains("master layer"))
    }));
}
