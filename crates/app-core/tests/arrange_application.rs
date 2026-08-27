use app_core::{ApplicationSession, ArrangeOperation};
use ddnx::PackageLimits;
use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene, Size,
};

fn rectangle(id: ElementId, x: f64, y: f64, width: f64) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm: Rect {
            x,
            y,
            width,
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
    }
}

fn fixture() -> (ApplicationSession, [ElementId; 3]) {
    let ids = [ElementId::new(), ElementId::new(), ElementId::new()];
    let document = Document {
        id: DocumentId::new(),
        name: "Arrange application".to_owned(),
        defaults: DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Transparent,
        },
        master_layers: Vec::new(),
        pages: vec![Page {
            id: PageId::new(),
            name: "Page 1".to_owned(),
            size_mm: Size {
                width: 210.0,
                height: 297.0,
            },
            layers: vec![Layer {
                id: LayerId::new(),
                name: "Layer 1".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: ids.to_vec(),
                    elements: vec![
                        rectangle(ids[0], 10.0, 10.0, 10.0),
                        rectangle(ids[1], 50.0, 30.0, 20.0),
                        rectangle(ids[2], 100.0, 60.0, 10.0),
                    ],
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (
        ApplicationSession::from_artifact(NextArtifact::document(document)).unwrap(),
        ids,
    )
}

fn x(app: &ApplicationSession, id: ElementId) -> f64 {
    app.session().document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == id)
        .unwrap()
        .bounds_mm
        .x
}

#[test]
fn arrange_round_trips_through_application_history_and_ddnx() {
    let (mut app, ids) = fixture();
    let initial = app.session().current_history_state();
    assert!(
        app.arrange_elements(ids.to_vec(), ArrangeOperation::AlignHorizontalCenter)
            .unwrap()
    );
    let center0 = x(&app, ids[0]) + 5.0;
    let center1 = x(&app, ids[1]) + 10.0;
    let center2 = x(&app, ids[2]) + 5.0;
    assert!((center0 - center1).abs() < 1e-9);
    assert!((center1 - center2).abs() < 1e-9);
    let arranged = app.session().current_history_state();
    assert_ne!(arranged, initial);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened =
        ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(x(&reopened, ids[0]), x(&app, ids[0]));
    assert_eq!(x(&reopened, ids[1]), x(&app, ids[1]));
    assert_eq!(x(&reopened, ids[2]), x(&app, ids[2]));

    assert!(app.undo().unwrap());
    assert_eq!(app.session().current_history_state(), initial);
    assert_eq!(x(&app, ids[0]), 10.0);
    assert_eq!(x(&app, ids[1]), 50.0);
    assert!(app.redo().unwrap());
    assert_eq!(app.session().current_history_state(), arranged);
}
