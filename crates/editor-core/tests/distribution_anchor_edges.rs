use editor_core::{ArrangeOperation, EditCommand, EditorSession};
use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene, Size,
};

fn element(id: ElementId, bounds_mm: Rect) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm,
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

fn fixture(bounds: [Rect; 3]) -> (EditorSession, [ElementId; 3]) {
    let ids = [ElementId::new(), ElementId::new(), ElementId::new()];
    let page_id = PageId::new();
    let layer_id = LayerId::new();
    let document = Document {
        id: DocumentId::new(),
        name: "Distribution anchor edges".to_owned(),
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
                width: 300.0,
                height: 300.0,
            },
            layers: vec![Layer {
                id: layer_id,
                name: "Layer 1".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: ids.to_vec(),
                    elements: ids
                        .iter()
                        .zip(bounds)
                        .map(|(id, bounds_mm)| element(*id, bounds_mm))
                        .collect(),
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
fn horizontal_distribution_uses_the_farthest_trailing_edge_as_the_opposite_anchor() {
    let (mut session, ids) = fixture([
        Rect {
            x: 0.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
        },
        Rect {
            x: 30.0,
            y: 10.0,
            width: 100.0,
            height: 10.0,
        },
        Rect {
            x: 100.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
        },
    ]);

    assert!(session
        .execute(EditCommand::ArrangeElements {
            element_ids: vec![ids[2], ids[0], ids[1]],
            operation: ArrangeOperation::DistributeHorizontal,
        })
        .unwrap());

    // The true left and right visual anchors stay fixed even though the right
    // anchor starts before the third object's leading edge.
    assert_eq!(bounds(&session, ids[0]).x, 0.0);
    assert_eq!(bounds(&session, ids[1]).x, 30.0);
    assert_eq!(bounds(&session, ids[2]).x, 15.0);

    let first_gap = bounds(&session, ids[2]).x
        - (bounds(&session, ids[0]).x + bounds(&session, ids[0]).width);
    let second_gap = bounds(&session, ids[1]).x
        - (bounds(&session, ids[2]).x + bounds(&session, ids[2]).width);
    assert!((first_gap - 5.0).abs() < 1e-9);
    assert!((second_gap - 5.0).abs() < 1e-9);
}

#[test]
fn vertical_distribution_uses_the_farthest_trailing_edge_as_the_opposite_anchor() {
    let (mut session, ids) = fixture([
        Rect {
            x: 10.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        Rect {
            x: 10.0,
            y: 30.0,
            width: 10.0,
            height: 100.0,
        },
        Rect {
            x: 10.0,
            y: 100.0,
            width: 10.0,
            height: 10.0,
        },
    ]);

    assert!(session
        .execute(EditCommand::ArrangeElements {
            element_ids: vec![ids[1], ids[2], ids[0]],
            operation: ArrangeOperation::DistributeVertical,
        })
        .unwrap());

    assert_eq!(bounds(&session, ids[0]).y, 0.0);
    assert_eq!(bounds(&session, ids[1]).y, 30.0);
    assert_eq!(bounds(&session, ids[2]).y, 15.0);

    let first_gap = bounds(&session, ids[2]).y
        - (bounds(&session, ids[0]).y + bounds(&session, ids[0]).height);
    let second_gap = bounds(&session, ids[1]).y
        - (bounds(&session, ids[2]).y + bounds(&session, ids[2]).height);
    assert!((first_gap - 5.0).abs() < 1e-9);
    assert!((second_gap - 5.0).abs() < 1e-9);
}

#[test]
fn contained_selection_keeps_distinct_anchors_and_represents_overlap_as_equal_negative_gaps() {
    let (mut session, ids) = fixture([
        Rect {
            x: 0.0,
            y: 10.0,
            width: 200.0,
            height: 10.0,
        },
        Rect {
            x: 40.0,
            y: 10.0,
            width: 20.0,
            height: 10.0,
        },
        Rect {
            x: 100.0,
            y: 10.0,
            width: 20.0,
            height: 10.0,
        },
    ]);

    assert!(session
        .execute(EditCommand::ArrangeElements {
            element_ids: vec![ids[2], ids[1], ids[0]],
            operation: ArrangeOperation::DistributeHorizontal,
        })
        .unwrap());

    assert_eq!(bounds(&session, ids[0]).x, 0.0);
    assert_eq!(bounds(&session, ids[2]).x, 100.0);
    assert_eq!(bounds(&session, ids[1]).x, 140.0);

    let first_overlap = bounds(&session, ids[1]).x
        - (bounds(&session, ids[0]).x + bounds(&session, ids[0]).width);
    let second_overlap = bounds(&session, ids[2]).x
        - (bounds(&session, ids[1]).x + bounds(&session, ids[1]).width);
    assert!((first_overlap + 60.0).abs() < 1e-9);
    assert!((second_overlap + 60.0).abs() < 1e-9);
}
