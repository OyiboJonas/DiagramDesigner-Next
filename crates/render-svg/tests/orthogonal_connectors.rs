use next_domain::{
    AnchorSet, Color, Connection, Connector, ConnectorLabelStyle, Document, DocumentDefaults,
    DocumentId, Element, ElementId, ElementKind, ElementStyle, Endpoint, Layer, LayerId, LineStyle,
    MarkerStyle, Page, Point, Port, PortId, Rect, Scene, Size, StrokeStyle, StyleId,
};
use render_plan::{RenderPlanOptions, build_page_plan};
use render_svg::{SvgDiagnostic, SvgRenderOptions, render_plan_to_svg};

fn defaults() -> DocumentDefaults {
    DocumentDefaults {
        font_family: "Arial".to_owned(),
        font_size_pt: 10.0,
        font_style_bits: 0,
        object_shadows: false,
        auto_line_break: true,
        connector_label_style: ConnectorLabelStyle::Transparent,
    }
}

fn rectangle(id: ElementId, bounds_mm: Rect, ports: Vec<Port>) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm,
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports,
        style_id: None,
        text: None,
        kind: ElementKind::Rectangle {
            corner_radius_mm: 0.0,
        },
        import: None,
    }
}

fn orthogonal(
    id: ElementId,
    start: Endpoint,
    end: Endpoint,
    start_marker: MarkerStyle,
    end_marker: MarkerStyle,
    line_style: LineStyle,
    corner_radius_mm: f64,
    style_id: Option<StyleId>,
    secondary_color: Option<Color>,
) -> Element {
    let min_x = start.position_mm.x.min(end.position_mm.x);
    let min_y = start.position_mm.y.min(end.position_mm.y);
    Element {
        id,
        name: String::new(),
        bounds_mm: Rect {
            x: min_x,
            y: min_y,
            width: (end.position_mm.x - start.position_mm.x).abs(),
            height: (end.position_mm.y - start.position_mm.y).abs(),
        },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id,
        text: None,
        kind: ElementKind::OrthogonalConnector {
            connector: Connector {
                start,
                end,
                start_marker,
                end_marker,
                line_style,
                secondary_color,
            },
            corner_radius_mm,
        },
        import: None,
    }
}

fn document(elements: Vec<Element>, styles: Vec<ElementStyle>) -> (Document, next_domain::PageId) {
    let page_id = next_domain::PageId::new();
    let roots = elements.iter().map(|element| element.id).collect();
    (
        Document {
            id: DocumentId::new(),
            name: "Orthogonal regression".to_owned(),
            defaults: defaults(),
            master_layers: Vec::new(),
            pages: vec![Page {
                id: page_id,
                name: "Page".to_owned(),
                size_mm: Size {
                    width: 210.0,
                    height: 297.0,
                },
                layers: vec![Layer {
                    id: LayerId::new(),
                    name: "Layer".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene { roots, elements },
                }],
            }],
            styles,
            assets: Vec::new(),
            import: None,
        },
        page_id,
    )
}

fn render(document: &Document, page_id: next_domain::PageId) -> render_svg::SvgRenderOutput {
    let plan = build_page_plan(document, page_id, RenderPlanOptions::default()).unwrap();
    render_plan_to_svg(document, page_id, &plan, SvgRenderOptions::default()).unwrap()
}

#[test]
fn same_side_vertical_ports_keep_the_legacy_marker_clearance_hairpin() {
    let first_id = ElementId::new();
    let second_id = ElementId::new();
    let first_port = PortId::new();
    let second_port = PortId::new();
    let connector_id = ElementId::new();

    let first = rectangle(
        first_id,
        Rect {
            x: 10.0,
            y: 30.0,
            width: 20.0,
            height: 10.0,
        },
        vec![Port {
            id: first_port,
            index: 0,
            position: next_domain::NormalizedPoint { x: 0.5, y: 0.0 },
        }],
    );
    let second = rectangle(
        second_id,
        Rect {
            x: 10.0,
            y: 70.0,
            width: 20.0,
            height: 10.0,
        },
        vec![Port {
            id: second_port,
            index: 0,
            position: next_domain::NormalizedPoint { x: 0.5, y: 0.0 },
        }],
    );
    let connector = orthogonal(
        connector_id,
        Endpoint {
            position_mm: Point { x: 20.0, y: 30.0 },
            connection: Some(Connection {
                element_id: first_id,
                port_id: first_port,
            }),
        },
        Endpoint {
            position_mm: Point { x: 20.0, y: 70.0 },
            connection: Some(Connection {
                element_id: second_id,
                port_id: second_port,
            }),
        },
        MarkerStyle::Arrow3,
        MarkerStyle::Arrow3,
        LineStyle::Solid,
        0.0,
        None,
        None,
    );

    let (document, page_id) = document(vec![first, second, connector], Vec::new());
    let output = render(&document, page_id);

    assert_eq!(output.rendered_elements, 3);
    assert_eq!(output.skipped_elements, 0);
    assert!(output.svg.contains("data-ddn-start-direction=\"vertical-top\""));
    assert!(output.svg.contains("data-ddn-end-direction=\"vertical-top\""));
    assert!(
        output
            .svg
            .contains("d=\"M 20 30 L 20 26.825 L 20 70\"")
    );
    assert!(output.svg.contains("marker-start=\"url(#ddn-marker-"));
    assert!(output.svg.contains("marker-end=\"url(#ddn-marker-"));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::ConnectorMarkerDeferred { element_id, marker: MarkerStyle::Arrow3 }
            if *element_id == connector_id
    )));
}

#[test]
fn styled_vertical_route_preserves_the_upstream_midpoint_segment_reset() {
    let connector_id = ElementId::new();
    let connector = orthogonal(
        connector_id,
        Endpoint {
            position_mm: Point { x: 10.0, y: 10.0 },
            connection: None,
        },
        Endpoint {
            position_mm: Point { x: 50.0, y: 70.0 },
            connection: None,
        },
        MarkerStyle::None,
        MarkerStyle::None,
        LineStyle::Dotted1,
        8.0,
        None,
        None,
    );
    let (document, page_id) = document(vec![connector], Vec::new());
    let output = render(&document, page_id);

    for segment in 0..4 {
        assert!(
            output
                .svg
                .contains(&format!("data-ddn-segment=\"{segment}\""))
        );
    }
    assert!(output.svg.contains("stroke-dasharray="));
    assert!(!output.svg.contains(" A "));
}

#[test]
fn rounded_outline_route_reuses_secondary_paint_and_standard_markers() {
    let left_id = ElementId::new();
    let top_id = ElementId::new();
    let left_port = PortId::new();
    let top_port = PortId::new();
    let connector_id = ElementId::new();
    let style_id = StyleId::new();

    let left = rectangle(
        left_id,
        Rect {
            x: 10.0,
            y: 20.0,
            width: 20.0,
            height: 20.0,
        },
        vec![Port {
            id: left_port,
            index: 0,
            position: next_domain::NormalizedPoint { x: 1.0, y: 0.5 },
        }],
    );
    let top = rectangle(
        top_id,
        Rect {
            x: 60.0,
            y: 70.0,
            width: 20.0,
            height: 20.0,
        },
        vec![Port {
            id: top_port,
            index: 0,
            position: next_domain::NormalizedPoint { x: 0.5, y: 0.0 },
        }],
    );
    let connector = orthogonal(
        connector_id,
        Endpoint {
            position_mm: Point { x: 30.0, y: 30.0 },
            connection: Some(Connection {
                element_id: left_id,
                port_id: left_port,
            }),
        },
        Endpoint {
            position_mm: Point { x: 70.0, y: 70.0 },
            connection: Some(Connection {
                element_id: top_id,
                port_id: top_port,
            }),
        },
        MarkerStyle::Many,
        MarkerStyle::Arrow2,
        LineStyle::Outline,
        5.0,
        Some(style_id),
        Some(Color::Rgba {
            r: 200,
            g: 30,
            b: 40,
            a: 128,
        }),
    );
    let style = ElementStyle {
        id: style_id,
        stroke: Some(StrokeStyle {
            width_mm: 2.0,
            color: Color::Rgba {
                r: 10,
                g: 20,
                b: 30,
                a: 255,
            },
        }),
        fill: None,
        text_color: None,
    };
    let (document, page_id) = document(vec![left, top, connector], vec![style]);
    let output = render(&document, page_id);

    assert!(output.svg.contains("data-ddn-start-direction=\"horizontal-right\""));
    assert!(output.svg.contains("data-ddn-end-direction=\"vertical-top\""));
    assert!(output.svg.contains(" A 5 5 0 0 1 "));
    assert!(output.svg.contains("data-ddn-outline-inner="));
    assert!(output.svg.contains("stroke=\"#c81e28\" stroke-opacity=\"0.502\" stroke-width=\"1\""));

    let outer = output.svg.find("data-ddn-connector-outer=").unwrap();
    let inner = output.svg.find("data-ddn-outline-inner=").unwrap();
    let carrier = output.svg.find("data-ddn-marker-target=").unwrap();
    assert!(outer < inner && inner < carrier);
    assert!(output.svg.contains("data-ddn-marker-style=\"many\""));
    assert!(output.svg.contains("data-ddn-marker-style=\"arrow2\""));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive { element_id, .. }
            | SvgDiagnostic::ConnectorLineStyleApproximated { element_id, .. }
            | SvgDiagnostic::ConnectorMarkerDeferred { element_id, .. }
            if *element_id == connector_id
    )));
}

#[test]
fn custom_orthogonal_styles_remain_explicit_diagnostics() {
    let connector_id = ElementId::new();
    let connector = orthogonal(
        connector_id,
        Endpoint {
            position_mm: Point { x: 20.0, y: 20.0 },
            connection: None,
        },
        Endpoint {
            position_mm: Point { x: 80.0, y: 50.0 },
            connection: None,
        },
        MarkerStyle::Custom(0x72),
        MarkerStyle::None,
        LineStyle::Custom(0x71),
        0.0,
        None,
        None,
    );
    let (document, page_id) = document(vec![connector], Vec::new());
    let output = render(&document, page_id);

    assert_eq!(output.rendered_elements, 1);
    assert_eq!(output.skipped_elements, 0);
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::ConnectorLineStyleApproximated {
            element_id,
            line_style: LineStyle::Custom(0x71),
        } if *element_id == connector_id
    )));
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::ConnectorMarkerDeferred {
            element_id,
            marker: MarkerStyle::Custom(0x72),
        } if *element_id == connector_id
    )));
}
