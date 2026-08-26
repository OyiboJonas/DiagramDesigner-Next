use app_core::ApplicationSession;
use ddnx::PackageLimits;
use next_domain::{
    AnchorSet, Color, Connector, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId,
    Element, ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle,
    NextArtifact, Page, PageId, Point, Rect, Scene, Size,
};

fn connector_element(id: ElementId) -> Element {
    Element {
        id,
        name: "Connector".to_owned(),
        bounds_mm: Rect {
            x: 10.0,
            y: 10.0,
            width: 50.0,
            height: 30.0,
        },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::StraightConnector {
            connector: Connector {
                start: Endpoint {
                    position_mm: Point { x: 10.0, y: 10.0 },
                    connection: None,
                },
                end: Endpoint {
                    position_mm: Point { x: 60.0, y: 40.0 },
                    connection: None,
                },
                start_marker: MarkerStyle::None,
                end_marker: MarkerStyle::None,
                line_style: LineStyle::Solid,
                secondary_color: None,
            },
        },
        import: None,
    }
}

fn rectangle(id: ElementId) -> Element {
    Element {
        id,
        name: "Rectangle".to_owned(),
        bounds_mm: Rect {
            x: 70.0,
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

fn fixture() -> (NextArtifact, ElementId, ElementId) {
    let connector_id = ElementId::new();
    let rectangle_id = ElementId::new();
    let document = Document {
        id: DocumentId::new(),
        name: "Connector style application test".to_owned(),
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
                    roots: vec![connector_id, rectangle_id],
                    elements: vec![connector_element(connector_id), rectangle(rectangle_id)],
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (NextArtifact::document(document), connector_id, rectangle_id)
}

fn assert_updated(app: &ApplicationSession, connector_id: ElementId) {
    let state = app.connector_endpoints(connector_id).unwrap().unwrap();
    assert_eq!(state.start_marker, MarkerStyle::Arrow1);
    assert_eq!(state.end_marker, MarkerStyle::Arrow2);
    assert_eq!(state.line_style, LineStyle::Outline);
    assert_eq!(
        state.secondary_color,
        Some(Color::Rgba {
            r: 12,
            g: 34,
            b: 56,
            a: 255
        })
    );
}

#[test]
fn connector_style_round_trips_through_history_and_ddnx() {
    let (artifact, connector_id, rectangle_id) = fixture();
    let mut app = ApplicationSession::from_artifact(artifact).unwrap();
    let initial_history = app.session().current_history_state();

    assert!(
        app.set_connector_style(
            connector_id,
            MarkerStyle::Arrow1,
            MarkerStyle::Arrow2,
            LineStyle::Outline,
            Some(Color::Rgba {
                r: 12,
                g: 34,
                b: 56,
                a: 255
            }),
        )
        .unwrap()
    );
    assert_updated(&app, connector_id);
    let styled_history = app.session().current_history_state();
    assert_ne!(styled_history, initial_history);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened =
        ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_updated(&reopened, connector_id);

    assert!(app.undo().unwrap());
    let restored = app.connector_endpoints(connector_id).unwrap().unwrap();
    assert_eq!(restored.start_marker, MarkerStyle::None);
    assert_eq!(restored.end_marker, MarkerStyle::None);
    assert_eq!(restored.line_style, LineStyle::Solid);
    assert_eq!(restored.secondary_color, None);
    assert_eq!(app.session().current_history_state(), initial_history);

    assert!(app.redo().unwrap());
    assert_updated(&app, connector_id);
    assert_eq!(app.session().current_history_state(), styled_history);

    let before_noop = app.session().current_history_state();
    assert!(
        !app.set_connector_style(
            connector_id,
            MarkerStyle::Arrow1,
            MarkerStyle::Arrow2,
            LineStyle::Outline,
            Some(Color::Rgba {
                r: 12,
                g: 34,
                b: 56,
                a: 255
            }),
        )
        .unwrap()
    );
    assert_eq!(app.session().current_history_state(), before_noop);

    assert!(
        app.set_connector_style(
            rectangle_id,
            MarkerStyle::None,
            MarkerStyle::None,
            LineStyle::Solid,
            None,
        )
        .is_err()
    );
}
