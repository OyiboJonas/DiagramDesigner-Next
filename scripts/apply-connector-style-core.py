#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace(path, old, new):
    p = ROOT / path
    text = p.read_text(encoding='utf-8')
    if old not in text:
        raise SystemExit(f'pattern not found in {path}: {old[:120]!r}')
    text = text.replace(old, new, 1)
    p.write_text(text, encoding='utf-8')


# editor-core imports and public connector snapshot
replace(
    'crates/editor-core/src/lib.rs',
    '    ElementStyle, Endpoint, FillStyle, Layer, LayerId, NextArtifact, Page, PageId, Point, PortId,\n    Rect, Scene, Size, StrokeStyle, StyleId, TextBlock, ValidationReport,\n',
    '    ElementStyle, Endpoint, FillStyle, Layer, LayerId, LineStyle, MarkerStyle, NextArtifact, Page,\n    PageId, Point, PortId, Rect, Scene, Size, StrokeStyle, StyleId, TextBlock, ValidationReport,\n',
)
replace(
    'crates/editor-core/src/lib.rs',
    'pub struct ConnectorEndpointSnapshot {\n    pub kind: ConnectorGeometryKind,\n    pub start: Endpoint,\n    pub end: Endpoint,\n}\n',
    'pub struct ConnectorEndpointSnapshot {\n    pub kind: ConnectorGeometryKind,\n    pub start: Endpoint,\n    pub end: Endpoint,\n    pub start_marker: MarkerStyle,\n    pub end_marker: MarkerStyle,\n    pub line_style: LineStyle,\n    pub secondary_color: Option<Color>,\n}\n',
)

# typed command
replace(
    'crates/editor-core/src/lib.rs',
    '    SetConnectorEndpoint {\n        element_id: ElementId,\n        side: ConnectorEndpointSide,\n        position_mm: Point,\n        connection: Option<Connection>,\n    },\n    SetElementStyle {\n',
    '    SetConnectorEndpoint {\n        element_id: ElementId,\n        side: ConnectorEndpointSide,\n        position_mm: Point,\n        connection: Option<Connection>,\n    },\n    /// Replace persisted connector paint semantics as one history command.\n    SetConnectorStyle {\n        element_id: ElementId,\n        start_marker: MarkerStyle,\n        end_marker: MarkerStyle,\n        line_style: LineStyle,\n        secondary_color: Option<Color>,\n    },\n    SetElementStyle {\n',
)

# undo snapshot
replace(
    'crates/editor-core/src/lib.rs',
    '    SetConnectorEndpoint {\n        element_id: ElementId,\n        side: ConnectorEndpointSide,\n        endpoint: Endpoint,\n    },\n    SetElementStyles {\n',
    '    SetConnectorEndpoint {\n        element_id: ElementId,\n        side: ConnectorEndpointSide,\n        endpoint: Endpoint,\n    },\n    SetConnectorStyle {\n        element_id: ElementId,\n        start_marker: MarkerStyle,\n        end_marker: MarkerStyle,\n        line_style: LineStyle,\n        secondary_color: Option<Color>,\n    },\n    SetElementStyles {\n',
)

# public snapshot values
replace(
    'crates/editor-core/src/lib.rs',
    '        Ok(Some(ConnectorEndpointSnapshot {\n            kind,\n            start: connector.start.clone(),\n            end: connector.end.clone(),\n        }))\n',
    '        Ok(Some(ConnectorEndpointSnapshot {\n            kind,\n            start: connector.start.clone(),\n            end: connector.end.clone(),\n            start_marker: connector.start_marker,\n            end_marker: connector.end_marker,\n            line_style: connector.line_style,\n            secondary_color: connector.secondary_color,\n        }))\n',
)

# command dispatch
replace(
    'crates/editor-core/src/lib.rs',
    '        EditCommand::SetConnectorEndpoint {\n            element_id,\n            side,\n            position_mm,\n            connection,\n        } => apply_set_connector_endpoint(document, *element_id, *side, *position_mm, *connection),\n        EditCommand::SetElementStyle {\n',
    '        EditCommand::SetConnectorEndpoint {\n            element_id,\n            side,\n            position_mm,\n            connection,\n        } => apply_set_connector_endpoint(document, *element_id, *side, *position_mm, *connection),\n        EditCommand::SetConnectorStyle {\n            element_id,\n            start_marker,\n            end_marker,\n            line_style,\n            secondary_color,\n        } => apply_set_connector_style(\n            document,\n            *element_id,\n            *start_marker,\n            *end_marker,\n            *line_style,\n            *secondary_color,\n        ),\n        EditCommand::SetElementStyle {\n',
)

# apply function
replace(
    'crates/editor-core/src/lib.rs',
    '        // Connection references participate in Next-domain structural validation.\n        structural: true,\n    }))\n}\n\nfn apply_set_element_style(\n',
    '        // Connection references participate in Next-domain structural validation.\n        structural: true,\n    }))\n}\n\nfn apply_set_connector_style(\n    document: &mut Document,\n    element_id: ElementId,\n    start_marker: MarkerStyle,\n    end_marker: MarkerStyle,\n    line_style: LineStyle,\n    secondary_color: Option<Color>,\n) -> Result<Option<AppliedCommand>, EditorError> {\n    ensure_element_editable(document, element_id)?;\n    let (previous_start_marker, previous_end_marker, previous_line_style, previous_secondary_color) = {\n        let element =\n            find_element(document, element_id).ok_or(EditorError::ElementNotFound(element_id))?;\n        let connector = connector(element).ok_or(EditorError::ElementIsNotConnector(element_id))?;\n        (\n            connector.start_marker,\n            connector.end_marker,\n            connector.line_style,\n            connector.secondary_color,\n        )\n    };\n\n    if previous_start_marker == start_marker\n        && previous_end_marker == end_marker\n        && previous_line_style == line_style\n        && previous_secondary_color == secondary_color\n    {\n        return Ok(None);\n    }\n\n    let element =\n        find_element_mut(document, element_id).ok_or(EditorError::HistoryInvariantViolation)?;\n    let connector = connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?;\n    connector.start_marker = start_marker;\n    connector.end_marker = end_marker;\n    connector.line_style = line_style;\n    connector.secondary_color = secondary_color;\n\n    Ok(Some(AppliedCommand {\n        undo: UndoStep::SetConnectorStyle {\n            element_id,\n            start_marker: previous_start_marker,\n            end_marker: previous_end_marker,\n            line_style: previous_line_style,\n            secondary_color: previous_secondary_color,\n        },\n        structural: false,\n    }))\n}\n\nfn apply_set_element_style(\n',
)

# undo restore
replace(
    'crates/editor-core/src/lib.rs',
    '            refresh_connector_bounds(document, *element_id)?;\n            synchronize_connected_endpoints(document)?;\n        }\n        UndoStep::SetElementStyles { previous } => {\n',
    '            refresh_connector_bounds(document, *element_id)?;\n            synchronize_connected_endpoints(document)?;\n        }\n        UndoStep::SetConnectorStyle {\n            element_id,\n            start_marker,\n            end_marker,\n            line_style,\n            secondary_color,\n        } => {\n            let element = find_element_mut(document, *element_id)\n                .ok_or(EditorError::HistoryInvariantViolation)?;\n            let connector = connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?;\n            connector.start_marker = *start_marker;\n            connector.end_marker = *end_marker;\n            connector.line_style = *line_style;\n            connector.secondary_color = *secondary_color;\n        }\n        UndoStep::SetElementStyles { previous } => {\n',
)

# app-core imports, DTO and mapping
replace(
    'crates/app-core/src/lib.rs',
    '    Color, Connection, Element, ElementId, ElementKind, FillStyle, Layer, LayerId, NextArtifact,\n    Page, PageId, Point, PortId, Rect, Size, StrokeStyle, TextBlock,\n',
    '    Color, Connection, Element, ElementId, ElementKind, FillStyle, Layer, LayerId, LineStyle,\n    MarkerStyle, NextArtifact, Page, PageId, Point, PortId, Rect, Size, StrokeStyle, TextBlock,\n',
)
replace(
    'crates/app-core/src/lib.rs',
    'pub struct ConnectorEndpoints {\n    pub kind: ConnectorGeometryKind,\n    pub start: ConnectorEndpointState,\n    pub end: ConnectorEndpointState,\n}\n',
    'pub struct ConnectorEndpoints {\n    pub kind: ConnectorGeometryKind,\n    pub start: ConnectorEndpointState,\n    pub end: ConnectorEndpointState,\n    pub start_marker: MarkerStyle,\n    pub end_marker: MarkerStyle,\n    pub line_style: LineStyle,\n    pub secondary_color: Option<Color>,\n}\n',
)
replace(
    'crates/app-core/src/lib.rs',
    '            end: ConnectorEndpointState {\n                position_mm: value.end.position_mm,\n                connection: value.end.connection,\n            },\n        }\n',
    '            end: ConnectorEndpointState {\n                position_mm: value.end.position_mm,\n                connection: value.end.connection,\n            },\n            start_marker: value.start_marker,\n            end_marker: value.end_marker,\n            line_style: value.line_style,\n            secondary_color: value.secondary_color,\n        }\n',
)
replace(
    'crates/app-core/src/lib.rs',
    '    /// Read one connector\'s canonical endpoint state without exposing editor-core types.\n    pub fn connector_endpoints(\n',
    '    /// Replace connector marker/line semantics as one persistent history step.\n    pub fn set_connector_style(\n        &mut self,\n        element_id: ElementId,\n        start_marker: MarkerStyle,\n        end_marker: MarkerStyle,\n        line_style: LineStyle,\n        secondary_color: Option<Color>,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::SetConnectorStyle {\n            element_id,\n            start_marker,\n            end_marker,\n            line_style,\n            secondary_color,\n        })\n    }\n\n    /// Read one connector\'s canonical endpoint state without exposing editor-core types.\n    pub fn connector_endpoints(\n',
)

# app-core integration test
(ROOT / 'crates/app-core/tests/connector_style_application.rs').write_text(r'''use app_core::ApplicationSession;
use ddnx::PackageLimits;
use next_domain::{
    AnchorSet, Color, Connector, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId,
    Element, ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle, NextArtifact,
    Page, PageId, Point, Rect, Scene, Size,
};

fn connector_element(id: ElementId) -> Element {
    Element {
        id,
        name: "Connector".to_owned(),
        bounds_mm: Rect { x: 10.0, y: 10.0, width: 50.0, height: 30.0 },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::StraightConnector {
            connector: Connector {
                start: Endpoint { position_mm: Point { x: 10.0, y: 10.0 }, connection: None },
                end: Endpoint { position_mm: Point { x: 60.0, y: 40.0 }, connection: None },
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
        bounds_mm: Rect { x: 70.0, y: 20.0, width: 20.0, height: 15.0 },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::Rectangle { corner_radius_mm: 0.0 },
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
            size_mm: Size { width: 210.0, height: 297.0 },
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
        Some(Color::Rgba { r: 12, g: 34, b: 56, a: 255 })
    );
}

#[test]
fn connector_style_round_trips_through_history_and_ddnx() {
    let (artifact, connector_id, rectangle_id) = fixture();
    let mut app = ApplicationSession::from_artifact(artifact).unwrap();
    let initial_history = app.session().current_history_state();

    assert!(app
        .set_connector_style(
            connector_id,
            MarkerStyle::Arrow1,
            MarkerStyle::Arrow2,
            LineStyle::Outline,
            Some(Color::Rgba { r: 12, g: 34, b: 56, a: 255 }),
        )
        .unwrap());
    assert_updated(&app, connector_id);
    let styled_history = app.session().current_history_state();
    assert_ne!(styled_history, initial_history);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened = ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
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
    assert!(!app
        .set_connector_style(
            connector_id,
            MarkerStyle::Arrow1,
            MarkerStyle::Arrow2,
            LineStyle::Outline,
            Some(Color::Rgba { r: 12, g: 34, b: 56, a: 255 }),
        )
        .unwrap());
    assert_eq!(app.session().current_history_state(), before_noop);

    assert!(app
        .set_connector_style(
            rectangle_id,
            MarkerStyle::None,
            MarkerStyle::None,
            LineStyle::Solid,
            None,
        )
        .is_err());
}
''', encoding='utf-8')
