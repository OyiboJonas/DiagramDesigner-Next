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
    start_marker: MarkerStyle,
    end_marker: MarkerStyle,
    line_style: LineStyle,
    secondary_color: Option<Color>,
    style_id: Option<StyleId>,
) -> Element {
    Element {
        id: ElementId::new(),
        name: "connector".to_owned(),
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

fn document(elements: Vec<Element>, styles: Vec<ElementStyle>) -> (Document, PageId) {
    let page_id = PageId::new();
    let roots = elements.iter().map(|element| element.id).collect();
    (
        Document {
            id: DocumentId::new(),
            name: "marker-test".to_owned(),
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

#[test]
fn renders_every_standard_marker_without_deferred_diagnostics() {
    let markers = [
        MarkerStyle::Stop,
        MarkerStyle::Circle,
        MarkerStyle::Ball,
        MarkerStyle::Diamond,
        MarkerStyle::Arrow1,
        MarkerStyle::Arrow2,
        MarkerStyle::Arrow3,
        MarkerStyle::DoubleArrow,
        MarkerStyle::UmlIsA,
        MarkerStyle::UmlHasA,
        MarkerStyle::Many,
    ];
    let elements = markers
        .iter()
        .enumerate()
        .map(|(index, marker)| {
            connector_element(
                10.0 + index as f64 * 32.0,
                MarkerStyle::None,
                *marker,
                LineStyle::Solid,
                None,
                None,
            )
        })
        .collect();
    let (document, page_id) = document(elements, Vec::new());
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output = render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::ConnectorMarkerDeferred { .. }
    )));
    for name in [
        "stop",
        "circle",
        "ball",
        "diamond",
        "arrow1",
        "arrow2",
        "arrow3",
        "double-arrow",
        "uml-is-a",
        "uml-has-a",
        "many",
    ] {
        assert!(
            output
                .svg
                .contains(&format!("data-ddn-marker-style=\"{name}\"")),
            "missing marker {name}"
        );
    }
    assert_eq!(output.svg.matches(" marker-end=\"url(#ddn-marker-").count(), 11);
}

#[test]
fn supports_start_and_end_markers_with_auto_start_reverse_orientation() {
    let connector = connector_element(
        10.0,
        MarkerStyle::Arrow1,
        MarkerStyle::Diamond,
        LineStyle::Solid,
        None,
        None,
    );
    let element_id = connector.id;
    let (document, page_id) = document(vec![connector], Vec::new());
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output = render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert!(output.svg.contains(&format!(
        "<line data-element-id=\"{}\" marker-start=\"url(#ddn-marker-{}-start)\" marker-end=\"url(#ddn-marker-{}-end)\"",
        element_id.0, element_id.0, element_id.0
    )));
    assert_eq!(output.svg.matches("orient=\"auto-start-reverse\"").count(), 2);
}

#[test]
fn keeps_custom_markers_explicitly_deferred() {
    let connector = connector_element(
        10.0,
        MarkerStyle::None,
        MarkerStyle::Custom(0x77),
        LineStyle::Solid,
        None,
        None,
    );
    let element_id = connector.id;
    let (document, page_id) = document(vec![connector], Vec::new());
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output = render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::ConnectorMarkerDeferred {
            element_id: diagnostic_element_id,
            marker: MarkerStyle::Custom(0x77),
        } if *diagnostic_element_id == element_id
    )));
    assert!(!output.svg.contains("data-ddn-marker-style=\"custom"));
}

#[test]
fn uml_and_outline_marker_interiors_use_secondary_color() {
    let style_id = StyleId::new();
    let style = ElementStyle {
        id: style_id,
        stroke: Some(StrokeStyle {
            width_mm: 0.8,
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
    let secondary = Some(Color::Rgba {
        r: 240,
        g: 120,
        b: 20,
        a: 128,
    });
    let uml = connector_element(
        10.0,
        MarkerStyle::None,
        MarkerStyle::UmlIsA,
        LineStyle::Solid,
        secondary.clone(),
        Some(style_id),
    );
    let outline = connector_element(
        60.0,
        MarkerStyle::None,
        MarkerStyle::Diamond,
        LineStyle::Outline,
        secondary,
        Some(style_id),
    );
    let (document, page_id) = document(vec![uml, outline], vec![style]);
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output = render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert!(output.svg.matches("fill=\"#f07814\"").count() >= 2);
    assert!(output.svg.matches("fill-opacity=\"0.502\"").count() >= 2);
    // Outline line rendering itself is still an explicitly typed approximation;
    // this test only promotes the legacy marker-interior colour contract.
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::ConnectorLineStyleApproximated { .. }
    )));
}
