use app_core::{ApplicationSession, ZOrderOperation};
use ddnx::PackageLimits;
use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene, Size,
};

fn rectangle(id: ElementId, name: &str, x: f64) -> Element {
    Element {
        id,
        name: name.to_owned(),
        bounds_mm: Rect {
            x,
            y: 20.0,
            width: 25.0,
            height: 15.0,
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

fn fixture() -> (NextArtifact, [ElementId; 3]) {
    let ids = [ElementId::new(), ElementId::new(), ElementId::new()];
    let document = Document {
        id: DocumentId::new(),
        name: "Z-order application test".to_owned(),
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
                        rectangle(ids[0], "Back", 10.0),
                        rectangle(ids[1], "Middle", 20.0),
                        rectangle(ids[2], "Front", 30.0),
                    ],
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (NextArtifact::document(document), ids)
}

fn roots(app: &ApplicationSession) -> Vec<ElementId> {
    app.session().document().pages[0].layers[0]
        .scene
        .roots
        .clone()
}

#[test]
fn structural_group_z_order_round_trips_without_changing_children() {
    let (artifact, ids) = fixture();
    let mut app = ApplicationSession::from_artifact(artifact).unwrap();
    let group_id = ElementId::new();
    assert!(
        app.group_elements(group_id, vec![ids[0], ids[1]], "Pair".to_owned())
            .unwrap()
    );
    assert_eq!(roots(&app), vec![group_id, ids[2]]);

    let group_children = |app: &ApplicationSession| {
        let group = app.session().document().pages[0].layers[0]
            .scene
            .elements
            .iter()
            .find(|element| element.id == group_id)
            .unwrap();
        let ElementKind::Group { children } = &group.kind else {
            panic!("expected structural group")
        };
        children.clone()
    };
    assert_eq!(group_children(&app), vec![ids[0], ids[1]]);
    let grouped_history = app.session().current_history_state();

    assert!(
        app.reorder_elements(vec![group_id], ZOrderOperation::BringToFront)
            .unwrap()
    );
    assert_eq!(roots(&app), vec![ids[2], group_id]);
    assert_eq!(group_children(&app), vec![ids[0], ids[1]]);
    let reordered_history = app.session().current_history_state();
    assert_ne!(reordered_history, grouped_history);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened =
        ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(roots(&reopened), vec![ids[2], group_id]);
    assert_eq!(group_children(&reopened), vec![ids[0], ids[1]]);

    assert!(app.undo().unwrap());
    assert_eq!(app.session().current_history_state(), grouped_history);
    assert_eq!(roots(&app), vec![group_id, ids[2]]);
    assert_eq!(group_children(&app), vec![ids[0], ids[1]]);
    assert!(app.redo().unwrap());
    assert_eq!(app.session().current_history_state(), reordered_history);
    assert_eq!(roots(&app), vec![ids[2], group_id]);
    assert_eq!(group_children(&app), vec![ids[0], ids[1]]);

    let before_noop = app.session().current_history_state();
    assert!(
        !app.reorder_elements(vec![group_id], ZOrderOperation::BringToFront)
            .unwrap()
    );
    assert_eq!(app.session().current_history_state(), before_noop);
}

#[test]
fn z_order_round_trips_through_application_history_and_ddnx() {
    let (artifact, ids) = fixture();
    let mut app = ApplicationSession::from_artifact(artifact).unwrap();
    let initial_history = app.session().current_history_state();

    assert!(
        app.reorder_elements(vec![ids[0]], ZOrderOperation::BringToFront)
            .unwrap()
    );
    assert_eq!(roots(&app), vec![ids[1], ids[2], ids[0]]);
    assert!(app.is_dirty());
    let reordered_history = app.session().current_history_state();
    assert_ne!(reordered_history, initial_history);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened =
        ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(roots(&reopened), vec![ids[1], ids[2], ids[0]]);

    assert!(app.undo().unwrap());
    assert_eq!(roots(&app), ids.to_vec());
    assert_eq!(app.session().current_history_state(), initial_history);

    assert!(app.redo().unwrap());
    assert_eq!(roots(&app), vec![ids[1], ids[2], ids[0]]);
    assert_eq!(app.session().current_history_state(), reordered_history);

    let before_noop = app.session().current_history_state();
    assert!(
        !app.reorder_elements(vec![ids[0]], ZOrderOperation::BringToFront)
            .unwrap()
    );
    assert_eq!(app.session().current_history_state(), before_noop);
}
