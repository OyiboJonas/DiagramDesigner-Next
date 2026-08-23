use ddnx::{PackageLimits, prepare_package, read_package, write_package_to_vec};
use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene, Size,
};

fn document_with_master_layer() -> NextArtifact {
    let master_element_id = ElementId::new();
    NextArtifact::document(Document {
        id: DocumentId::new(),
        name: "Master layer round-trip".to_owned(),
        defaults: DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Solid,
        },
        master_layers: vec![Layer {
            id: LayerId::new(),
            name: "Shared background".to_owned(),
            visible: true,
            locked: false,
            draw_color: None,
            scene: Scene {
                roots: vec![master_element_id],
                elements: vec![Element {
                    id: master_element_id,
                    name: "Master rectangle".to_owned(),
                    bounds_mm: Rect {
                        x: 5.0,
                        y: 6.0,
                        width: 20.0,
                        height: 10.0,
                    },
                    rotation_deg: 0.0,
                    anchors: AnchorSet::default(),
                    ports: Vec::new(),
                    style_id: None,
                    text: None,
                    kind: ElementKind::Rectangle {
                        corner_radius_mm: 0.0,
                    },
                    import: None,
                }],
            },
        }],
        pages: vec![Page {
            id: PageId::new(),
            name: "Page 1".to_owned(),
            size_mm: Size {
                width: 210.0,
                height: 297.0,
            },
            layers: Vec::new(),
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    })
}

#[test]
fn document_master_layer_survives_native_package_round_trip() {
    let artifact = document_with_master_layer();
    assert!(artifact.validate().is_valid());

    let limits = PackageLimits::default();
    let prepared = prepare_package(&artifact, limits).unwrap();
    let bytes = write_package_to_vec(&prepared, limits).unwrap();
    let hydrated = read_package(&bytes, limits)
        .unwrap()
        .into_artifact()
        .unwrap();

    assert_eq!(hydrated, artifact);
}
