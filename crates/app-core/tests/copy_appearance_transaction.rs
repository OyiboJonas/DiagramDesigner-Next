use app_core::{ApplicationSession, ElementAppearanceUpdate};
use next_domain::{
    AnchorSet, Color, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,
    ElementId, ElementKind, FillStyle, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene,
    Size, StrokeStyle, StyleId,
};

const APPEARANCE_STYLE_NAMESPACE: &str = "diagramdesigner-next:element-appearance";

fn fixture() -> NextArtifact {
    NextArtifact::document(Document {
        id: DocumentId::new(),
        name: "Copy appearance transaction".to_owned(),
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
                scene: Scene::default(),
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    })
}

#[test]
fn copied_element_and_dedicated_appearance_commit_as_one_history_step() {
    let mut app = ApplicationSession::from_artifact(fixture()).unwrap();
    let target = app.session().active_layer().unwrap();
    let element_id = ElementId::new();
    let initial_history = app.session().current_history_state();
    let element = Element {
        id: element_id,
        name: "Copied rectangle".to_owned(),
        bounds_mm: Rect {
            x: 15.0,
            y: 25.0,
            width: 40.0,
            height: 20.0,
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
    let stroke = StrokeStyle {
        width_mm: 0.7,
        color: Color::Rgba {
            r: 12,
            g: 34,
            b: 56,
            a: 255,
        },
    };
    let fill = FillStyle {
        color: Color::Rgba {
            r: 210,
            g: 220,
            b: 230,
            a: 255,
        },
        gradient: None,
    };

    assert!(
        app.create_elements(
            target,
            vec![element],
            vec![ElementAppearanceUpdate {
                element_id,
                stroke: Some(stroke.clone()),
                fill: Some(fill.clone()),
                text_color: None,
            }],
        )
        .unwrap()
    );

    let committed_history = app.session().current_history_state();
    assert_ne!(committed_history, initial_history);
    let document = app.session().document();
    let copied = document.pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == element_id)
        .unwrap();
    let expected_style = StyleId::v5(element_id.0, APPEARANCE_STYLE_NAMESPACE);
    assert_eq!(copied.style_id, Some(expected_style));
    let style = document
        .styles
        .iter()
        .find(|style| style.id == expected_style)
        .unwrap();
    assert_eq!(style.stroke.as_ref(), Some(&stroke));
    assert_eq!(style.fill.as_ref(), Some(&fill));

    assert!(app.undo().unwrap());
    assert_eq!(app.session().current_history_state(), initial_history);
    assert!(app.session().document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .all(|element| element.id != element_id));

    assert!(app.redo().unwrap());
    assert_eq!(app.session().current_history_state(), committed_history);
    let document = app.session().document();
    let copied = document.pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == element_id)
        .unwrap();
    assert_eq!(copied.style_id, Some(expected_style));
}
