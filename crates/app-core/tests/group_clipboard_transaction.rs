use app_core::{ApplicationSession, StructuralGroupCreation};
use ddnx::PackageLimits;
use editor_core::LayerTarget;
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
            width: 20.0,
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

fn fixture() -> (NextArtifact, LayerTarget) {
    let page_id = PageId::new();
    let layer_id = LayerId::new();
    let document = Document {
        id: DocumentId::new(),
        name: "Clipboard group transaction".to_owned(),
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
            id: page_id,
            name: "Page".to_owned(),
            size_mm: Size {
                width: 210.0,
                height: 297.0,
            },
            layers: vec![Layer {
                id: layer_id,
                name: "Layer".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: Vec::new(),
                    elements: Vec::new(),
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (
        NextArtifact::document(document),
        LayerTarget::Page { page_id, layer_id },
    )
}

fn roots(app: &ApplicationSession) -> Vec<ElementId> {
    app.session().document().pages[0].layers[0]
        .scene
        .roots
        .clone()
}

fn group_children(app: &ApplicationSession, group_id: ElementId) -> Vec<ElementId> {
    let element = app.session().document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == group_id)
        .unwrap();
    let ElementKind::Group { children } = &element.kind else {
        panic!("expected structural group")
    };
    children.clone()
}

#[test]
fn clipboard_hierarchy_is_one_transaction_and_round_trips_through_ddnx() {
    let (artifact, target) = fixture();
    let mut app = ApplicationSession::from_artifact(artifact).unwrap();
    let initial_history = app.session().current_history_state();
    let first = ElementId::new();
    let second = ElementId::new();
    let third = ElementId::new();
    let ordinary = ElementId::new();
    let inner = ElementId::new();
    let outer = ElementId::new();

    assert!(
        app.create_elements_with_groups(
            target,
            vec![
                rectangle(first, "First", 15.0),
                rectangle(second, "Second", 40.0),
                rectangle(third, "Third", 65.0),
                rectangle(ordinary, "Ordinary", 100.0),
            ],
            vec![
                StructuralGroupCreation {
                    group_id: inner,
                    element_ids: vec![first, second],
                    name: "Inner".to_owned(),
                },
                StructuralGroupCreation {
                    group_id: outer,
                    element_ids: vec![inner, third],
                    name: "Outer".to_owned(),
                },
            ],
            Vec::new(),
        )
        .unwrap()
    );
    assert_eq!(roots(&app), vec![outer, ordinary]);
    assert_eq!(group_children(&app, inner), vec![first, second]);
    assert_eq!(group_children(&app, outer), vec![inner, third]);
    let created_history = app.session().current_history_state();
    assert_ne!(created_history, initial_history);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened =
        ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(roots(&reopened), vec![outer, ordinary]);
    assert_eq!(group_children(&reopened, inner), vec![first, second]);
    assert_eq!(group_children(&reopened, outer), vec![inner, third]);

    assert!(app.undo().unwrap());
    assert_eq!(app.session().current_history_state(), initial_history);
    assert!(roots(&app).is_empty());
    assert!(
        app.session().document().pages[0].layers[0]
            .scene
            .elements
            .is_empty()
    );

    assert!(app.redo().unwrap());
    assert_eq!(app.session().current_history_state(), created_history);
    assert_eq!(roots(&app), vec![outer, ordinary]);
    assert_eq!(group_children(&app, inner), vec![first, second]);
    assert_eq!(group_children(&app, outer), vec![inner, third]);
}
