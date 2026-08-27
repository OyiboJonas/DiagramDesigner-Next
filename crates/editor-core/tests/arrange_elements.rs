use editor_core::{ArrangeOperation, EditCommand, EditorError, EditorSession};
use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene, Size,
};

fn element(id: ElementId, x: f64, y: f64, width: f64, height: f64) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm: Rect {
            x,
            y,
            width,
            height,
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

fn fixture() -> (EditorSession, [ElementId; 4]) {
    let ids = [
        ElementId::new(),
        ElementId::new(),
        ElementId::new(),
        ElementId::new(),
    ];
    let page_id = PageId::new();
    let layer_id = LayerId::new();
    let document = Document {
        id: DocumentId::new(),
        name: "Arrange".to_owned(),
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
            name: "Page 1".to_owned(),
            size_mm: Size {
                width: 210.0,
                height: 297.0,
            },
            layers: vec![Layer {
                id: layer_id,
                name: "Layer 1".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: ids.to_vec(),
                    elements: vec![
                        element(ids[0], 10.0, 10.0, 10.0, 10.0),
                        element(ids[1], 35.0, 25.0, 20.0, 10.0),
                        element(ids[2], 80.0, 45.0, 10.0, 20.0),
                        element(ids[3], 130.0, 80.0, 20.0, 20.0),
                    ],
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (
        EditorSession::from_artifact(NextArtifact::document(document)).unwrap(),
        ids,
    )
}

fn bounds(session: &EditorSession, id: ElementId) -> Rect {
    session.document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == id)
        .unwrap()
        .bounds_mm
}

#[test]
fn alignment_is_one_history_step_and_noop_is_not_history() {
    let (mut session, ids) = fixture();
    let initial = session.current_history_state();
    assert!(
        session
            .execute(EditCommand::ArrangeElements {
                element_ids: vec![ids[2], ids[0], ids[1]],
                operation: ArrangeOperation::AlignLeft,
            })
            .unwrap()
    );
    assert_eq!(bounds(&session, ids[0]).x, 10.0);
    assert_eq!(bounds(&session, ids[1]).x, 10.0);
    assert_eq!(bounds(&session, ids[2]).x, 10.0);
    let aligned = session.current_history_state();
    assert_ne!(aligned, initial);
    assert!(session.undo().unwrap());
    assert_eq!(session.current_history_state(), initial);
    assert_eq!(bounds(&session, ids[1]).x, 35.0);
    assert!(session.redo().unwrap());
    assert_eq!(session.current_history_state(), aligned);

    let before_noop = session.current_history_state();
    assert!(
        !session
            .execute(EditCommand::ArrangeElements {
                element_ids: vec![ids[1], ids[0], ids[2]],
                operation: ArrangeOperation::AlignLeft,
            })
            .unwrap()
    );
    assert_eq!(session.current_history_state(), before_noop);
}

#[test]
fn distribution_keeps_outer_items_fixed_and_ignores_caller_order() {
    let (mut session, ids) = fixture();
    assert!(
        session
            .execute(EditCommand::ArrangeElements {
                element_ids: vec![ids[3], ids[1], ids[0], ids[2]],
                operation: ArrangeOperation::DistributeHorizontal,
            })
            .unwrap()
    );
    assert_eq!(bounds(&session, ids[0]).x, 10.0);
    assert_eq!(bounds(&session, ids[3]).x, 130.0);
    let gap1 =
        bounds(&session, ids[1]).x - (bounds(&session, ids[0]).x + bounds(&session, ids[0]).width);
    let gap2 =
        bounds(&session, ids[2]).x - (bounds(&session, ids[1]).x + bounds(&session, ids[1]).width);
    let gap3 =
        bounds(&session, ids[3]).x - (bounds(&session, ids[2]).x + bounds(&session, ids[2]).width);
    assert!((gap1 - gap2).abs() < 1e-9);
    assert!((gap2 - gap3).abs() < 1e-9);
}

#[test]
fn structural_group_moves_as_one_logical_arrange_item_and_child_ids_are_rejected() {
    let (mut session, ids) = fixture();
    let group_id = ElementId::new();
    assert!(
        session
            .execute(EditCommand::GroupElements {
                group_id,
                element_ids: vec![ids[0], ids[1]],
                name: "Pair".to_owned(),
            })
            .unwrap()
    );
    let child_before = bounds(&session, ids[0]);
    assert!(
        session
            .execute(EditCommand::ArrangeElements {
                element_ids: vec![group_id, ids[3]],
                operation: ArrangeOperation::AlignRight,
            })
            .unwrap()
    );
    let group = bounds(&session, group_id);
    let last = bounds(&session, ids[3]);
    assert!(((group.x + group.width) - (last.x + last.width)).abs() < 1e-9);
    assert!(bounds(&session, ids[0]).x > child_before.x);

    let history = session.current_history_state();
    let error = session
        .execute(EditCommand::ArrangeElements {
            element_ids: vec![ids[0], ids[3]],
            operation: ArrangeOperation::AlignTop,
        })
        .unwrap_err();
    assert!(matches!(error, EditorError::ArrangeRequiresTopLevelElement(id) if id == ids[0]));
    assert_eq!(session.current_history_state(), history);
}

#[test]
fn arrange_enforces_operation_selection_minimum() {
    let (mut session, ids) = fixture();
    let error = session
        .execute(EditCommand::ArrangeElements {
            element_ids: vec![ids[0], ids[1]],
            operation: ArrangeOperation::DistributeVertical,
        })
        .unwrap_err();
    assert!(matches!(
        error,
        EditorError::ArrangeRequiresAtLeast {
            required: 3,
            actual: 2
        }
    ));
}
