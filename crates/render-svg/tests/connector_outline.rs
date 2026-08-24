use next_domain::{
    AnchorSet, Color, Connector, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId,
    Element, ElementId, ElementKind, ElementStyle, Endpoint, Layer, LayerId, LineStyle,
    MarkerStyle, Page, PageId, Point, Rect, Scene, Size, StrokeStyle, StyleId,
};
use render_plan::{RenderPlanOptions, build_page_plan};
use render_svg::{SvgDiagnostic, SvgRenderOptions, render_plan_to_svg};

fn defaults() -> DocumentDefaults {
    DocumentDefaults {
        font_family: "Inter".to_owned(),
        font_size_pt: 10.0,
        font_style_bits: 0,
        object_shadows: false,
        auto_line_break: true,
        connector_label_style: ConnectorLabelStyle::Transparent,
    }
}

fn connector_element(
    x: f64,
    line_style: LineStyle,
    secondary_color: Option<Color>,
    style_id: Option<StyleId>,
    start_marker: MarkerStyle,
    end_marker: MarkerStyle,
) -> Element {
    Element {
        id: ElementId::new(),
        name: "outline-connector".to_owned(),
        bounds_mm: Rect {
            x,
            y: 20.0,
            width: 25.0,
            height: 10.0,
        },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id,
        text: None,
        kind: ElementKind::StraightConnector {
            connector: Connector {
                start: Endpoint {
                    position_mm: Point { x, y: 20.0 },
                    connection: None,
                },
                end: Endpoint {
                    position_mm: Point {
                        x: x + 25.0,
                        y: 30.0,
                    },
                    connection: None,
                },
                start_marker,
                end_marker,
                line_style,
                secondary_color,
            },
        },
        import: None,
    }
}

fn style(width_mm: f64, color: Color) -> ElementStyle {
    ElementStyle {
        id: StyleId::new(),
        stroke: Some(StrokeStyle { width_mm, color }),
        fill: None,
        text_color: None,
    }
}

fn document(elements: Vec<Element>, styles: Vec<ElementStyle>) -> (Document, PageId) {
    let page_id = PageId::new();
    let roots = elements.iter().map(|element| element.id).collect();
    (
        Document {
            id: DocumentId::new(),
            name: "outline-test".to_owned(),
            defaults: defaults(),
            master_layers: Vec::new(),
            pages: vec![Page {
                id: page_id,
                name: "Page 1".to_owned(),
                size_mm: Size {
                    width: 420.0,
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

fn render(document: &Document, page_id: PageId) -> render_svg::SvgRenderOutput {
    let plan = build_page_plan(document, page_id, RenderPlanOptions::default()).unwrap();
    render_plan_to_svg(document, page_id, &plan, SvgRenderOptions::default()).unwrap()
}

#[test]
fn outline_renders_primary_outer_and_half_width_secondary_inner_pass() {
    let style = style(
        0.8,
        Color::Rgba {
            r: 10,
            g: 20,
            b: 30,
            a: 128,
        },
    );
    let style_id = style.id;
    let connector = connector_element(
        10.0,
        LineStyle::Outline,
        Some(Color::Rgba {
            r: 240,
            g: 120,
            b: 20,
            a: 128,
        }),
        Some(style_id),
        MarkerStyle::None,
        MarkerStyle::None,
    );
    let element_id = connector.id;
    let (document, page_id) = document(vec![connector], vec![style]);
    let output = render(&document, page_id);

    assert!(output.svg.contains(&format!(
        "<line data-element-id=\"{}\" x1=\"10\" y1=\"20\" x2=\"35\" y2=\"30\" fill=\"none\" stroke-linecap=\"round\" stroke=\"#0a141e\" stroke-opacity=\"0.502\" stroke-width=\"0.8\"",
        element_id.0
    )));
    assert!(output.svg.contains(&format!(
        "<line data-ddn-outline-inner=\"{}\" x1=\"10\" y1=\"20\" x2=\"35\" y2=\"30\" fill=\"none\" stroke-linecap=\"round\" stroke=\"#f07814\" stroke-opacity=\"0.502\" stroke-width=\"0.4\" pointer-events=\"none\"",
        element_id.0
    )));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::ConnectorLineStyleApproximated {
            element_id: diagnostic_element_id,
            line_style: LineStyle::Outline,
        } if *diagnostic_element_id == element_id
    )));
}

#[test]
fn outline_defaults_secondary_pass_to_legacy_white() {
    let style = style(
        1.0,
        Color::Rgba {
            r: 1,
            g: 2,
            b: 3,
            a: 255,
        },
    );
    let style_id = style.id;
    let connector = connector_element(
        10.0,
        LineStyle::Outline,
        None,
        Some(style_id),
        MarkerStyle::None,
        MarkerStyle::None,
    );
    let element_id = connector.id;
    let (document, page_id) = document(vec![connector], vec![style]);
    let output = render(&document, page_id);

    assert!(output.svg.contains(&format!(
        "data-ddn-outline-inner=\"{}\" x1=\"10\" y1=\"20\" x2=\"35\" y2=\"30\" fill=\"none\" stroke-linecap=\"round\" stroke=\"#ffffff\" stroke-width=\"0.5\"",
        element_id.0
    )));
}

#[test]
fn outline_secondary_system_palette_uses_typed_fallback() {
    let style = style(
        0.6,
        Color::Rgba {
            r: 12,
            g: 34,
            b: 56,
            a: 255,
        },
    );
    let style_id = style.id;
    let connector = connector_element(
        10.0,
        LineStyle::Outline,
        Some(Color::SystemPalette { index: 7 }),
        Some(style_id),
        MarkerStyle::None,
        MarkerStyle::None,
    );
    let element_id = connector.id;
    let (document, page_id) = document(vec![connector], vec![style]);
    let output = render(&document, page_id);

    assert!(output.svg.contains(&format!(
        "data-ddn-outline-inner=\"{}\" x1=\"10\" y1=\"20\" x2=\"35\" y2=\"30\" fill=\"none\" stroke-linecap=\"round\" stroke=\"#808080\" stroke-width=\"0.3\"",
        element_id.0
    )));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| matches!(
                diagnostic,
                SvgDiagnostic::SystemPaletteFallback {
                    element_id: diagnostic_element_id,
                    index: 7,
                } if *diagnostic_element_id == element_id
            ))
            .count(),
        1
    );
}

#[test]
fn outline_markers_are_carried_after_both_paint_passes() {
    let style = style(
        0.8,
        Color::Rgba {
            r: 10,
            g: 20,
            b: 30,
            a: 255,
        },
    );
    let style_id = style.id;
    let connector = connector_element(
        10.0,
        LineStyle::Outline,
        Some(Color::Rgba {
            r: 240,
            g: 120,
            b: 20,
            a: 255,
        }),
        Some(style_id),
        MarkerStyle::Arrow1,
        MarkerStyle::Diamond,
    );
    let element_id = connector.id;
    let (document, page_id) = document(vec![connector], vec![style]);
    let output = render(&document, page_id);

    let outer = format!("<line data-element-id=\"{}\"", element_id.0);
    let inner = format!("<line data-ddn-outline-inner=\"{}\"", element_id.0);
    let carrier = format!("<line data-ddn-marker-target=\"{}\"", element_id.0);
    let outer_at = output.svg.find(&outer).unwrap();
    let inner_at = output.svg.find(&inner).unwrap();
    let carrier_at = output.svg.find(&carrier).unwrap();

    assert!(outer_at < inner_at && inner_at < carrier_at);
    let outer_end = output.svg[outer_at..].find("/>").unwrap() + outer_at;
    assert!(!output.svg[outer_at..outer_end].contains("marker-start="));
    assert!(!output.svg[outer_at..outer_end].contains("marker-end="));
    let carrier_end = output.svg[carrier_at..].find("/>").unwrap() + carrier_at;
    assert!(output.svg[carrier_at..carrier_end].contains(&format!(
        "marker-start=\"url(#ddn-marker-{}-start)\"",
        element_id.0
    )));
    assert!(output.svg[carrier_at..carrier_end].contains(&format!(
        "marker-end=\"url(#ddn-marker-{}-end)\"",
        element_id.0
    )));
}

#[test]
fn outline_inner_pass_and_marker_carrier_follow_connector_rotation() {
    let style = style(
        0.8,
        Color::Rgba {
            r: 10,
            g: 20,
            b: 30,
            a: 255,
        },
    );
    let style_id = style.id;
    let mut connector = connector_element(
        10.0,
        LineStyle::Outline,
        None,
        Some(style_id),
        MarkerStyle::None,
        MarkerStyle::Arrow2,
    );
    connector.rotation_deg = 30.0;
    let element_id = connector.id;
    let (document, page_id) = document(vec![connector], vec![style]);
    let output = render(&document, page_id);

    assert_eq!(
        output
            .svg
            .matches("transform=\"rotate(30 22.5 25)\"")
            .count(),
        3
    );
    assert!(output.svg.contains(&format!(
        "data-ddn-marker-target=\"{}\" marker-end=\"url(#ddn-marker-{}-end)\"",
        element_id.0, element_id.0
    )));
}

#[test]
fn explicit_no_stroke_outline_is_supported_without_materializing_extra_passes() {
    let style_id = StyleId::new();
    let style = ElementStyle {
        id: style_id,
        stroke: None,
        fill: None,
        text_color: None,
    };
    let connector = connector_element(
        10.0,
        LineStyle::Outline,
        None,
        Some(style_id),
        MarkerStyle::None,
        MarkerStyle::None,
    );
    let element_id = connector.id;
    let (document, page_id) = document(vec![connector], vec![style]);
    let output = render(&document, page_id);

    assert!(output.svg.contains(&format!(
        "<line data-element-id=\"{}\" x1=\"10\" y1=\"20\" x2=\"35\" y2=\"30\" fill=\"none\" stroke-linecap=\"round\" stroke=\"none\"",
        element_id.0
    )));
    assert!(!output.svg.contains("data-ddn-outline-inner="));
    assert!(!output.svg.contains("data-ddn-marker-target="));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::ConnectorLineStyleApproximated {
            element_id: diagnostic_element_id,
            line_style: LineStyle::Outline,
        } if *diagnostic_element_id == element_id
    )));
}

#[test]
fn custom_line_style_remains_explicitly_approximated() {
    let style = style(
        0.8,
        Color::Rgba {
            r: 10,
            g: 20,
            b: 30,
            a: 255,
        },
    );
    let style_id = style.id;
    let connector = connector_element(
        10.0,
        LineStyle::Custom(0x77),
        None,
        Some(style_id),
        MarkerStyle::None,
        MarkerStyle::None,
    );
    let element_id = connector.id;
    let (document, page_id) = document(vec![connector], vec![style]);
    let output = render(&document, page_id);

    assert!(!output.svg.contains("data-ddn-outline-inner="));
    assert!(!output.svg.contains("data-ddn-marker-target="));
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::ConnectorLineStyleApproximated {
            element_id: diagnostic_element_id,
            line_style: LineStyle::Custom(0x77),
        } if *diagnostic_element_id == element_id
    )));
}
