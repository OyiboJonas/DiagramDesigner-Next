use editor_core::{EditCommand, EditorSession};
use next_domain::{
    AnchorSet, Color, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,
    ElementId, ElementKind, FillStyle, GradientAxis, Layer, LayerId, LinearGradient, NextArtifact,
    Page, PageId, Rect, Scene, Size, StrokeStyle,
};

fn fixture() -> (EditorSession, ElementId) {
    let element_id = ElementId::new();
    let page_id = PageId::new();
    let layer_id = LayerId::new();
    let element = Element {
        id: element_id,
        name: "Gradient rectangle".to_owned(),
        bounds_mm: Rect {
            x: 10.0,
            y: 20.0,
            width: 40.0,
            height: 25.0,
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
    };
    let document = Document {
        id: DocumentId::new(),
        name: "Gradient history".to_owned(),
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
                    roots: vec![element_id],
                    elements: vec![element],
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (
        EditorSession::from_artifact(NextArtifact::document(document)).unwrap(),
        element_id,
    )
}

fn element<'a>(session: &'a EditorSession, id: ElementId) -> &'a Element {
    session.document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == id)
        .unwrap()
}

#[test]
fn gradient_appearance_is_one_history_step_and_round_trips_undo_redo_losslessly() {
    let (mut session, element_id) = fixture();
    let before = session.current_history_state();
    let start_color = Color::SystemPalette { index: 3 };
    let end_color = Color::SystemPalette { index: 7 };

    assert!(session
        .execute(EditCommand::SetElementAppearance {
            element_id,
            stroke: Some(StrokeStyle {
                width_mm: 0.25,
                color: Color::Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            }),
            fill: Some(FillStyle {
                color: start_color,
                gradient: Some(LinearGradient {
                    end_color,
                    axis: GradientAxis::AlongY,
                }),
            }),
            text_color: None,
        })
        .unwrap());

    let after = session.current_history_state();
    assert_ne!(after, before);
    assert!(session.can_undo());
    let style_id = element(&session, element_id).style_id.unwrap();
    let style = session
        .document()
        .styles
        .iter()
        .find(|style| style.id == style_id)
        .unwrap();
    let fill = style.fill.as_ref().unwrap();
    assert_eq!(fill.color, start_color);
    assert_eq!(fill.gradient.as_ref().unwrap().end_color, end_color);
    assert_eq!(fill.gradient.as_ref().unwrap().axis, GradientAxis::AlongY);

    assert!(session.undo().unwrap());
    assert_eq!(session.current_history_state(), before);
    assert_eq!(element(&session, element_id).style_id, None);
    assert!(session
        .document()
        .styles
        .iter()
        .all(|style| style.id != style_id));

    assert!(session.redo().unwrap());
    assert_eq!(session.current_history_state(), after);
    let style = session
        .document()
        .styles
        .iter()
        .find(|style| style.id == style_id)
        .unwrap();
    let fill = style.fill.as_ref().unwrap();
    assert_eq!(fill.color, start_color);
    assert_eq!(fill.gradient.as_ref().unwrap().end_color, end_color);
    assert_eq!(fill.gradient.as_ref().unwrap().axis, GradientAxis::AlongY);
}
