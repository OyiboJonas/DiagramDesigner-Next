use next_domain::{
    AnchorSet, Color, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,
    ElementId, ElementKind, ElementStyle, FillStyle, GradientAxis, Layer, LayerId, LinearGradient,
    NormalizedPoint, Page, Rect, Scene, Size, StrokeStyle, StyleId,
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

fn element(id: ElementId, bounds_mm: Rect, kind: ElementKind) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm,
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind,
        import: None,
    }
}

fn polygon(id: ElementId, style_id: Option<StyleId>, vertices: Vec<NormalizedPoint>) -> Element {
    let mut polygon = element(
        id,
        Rect {
            x: 10.0,
            y: 20.0,
            width: 40.0,
            height: 30.0,
        },
        ElementKind::Polygon { vertices },
    );
    polygon.style_id = style_id;
    polygon
}

fn document(elements: Vec<Element>, styles: Vec<ElementStyle>) -> (Document, next_domain::PageId) {
    let page_id = next_domain::PageId::new();
    let roots = elements.iter().map(|element| element.id).collect();
    (
        Document {
            id: DocumentId::new(),
            name: "Polygon regression".to_owned(),
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
fn maps_normalized_vertices_into_bounds_and_preserves_plan_order_and_rotation() {
    let before_id = ElementId::new();
    let polygon_id = ElementId::new();
    let after_id = ElementId::new();

    let before = element(
        before_id,
        Rect {
            x: 1.0,
            y: 1.0,
            width: 5.0,
            height: 5.0,
        },
        ElementKind::Rectangle {
            corner_radius_mm: 0.0,
        },
    );
    let mut shape = polygon(
        polygon_id,
        None,
        vec![
            NormalizedPoint { x: 0.0, y: 0.0 },
            NormalizedPoint { x: 1.0, y: 0.0 },
            NormalizedPoint { x: 0.5, y: 1.0 },
        ],
    );
    shape.rotation_deg = 30.0;
    let after = element(
        after_id,
        Rect {
            x: 70.0,
            y: 70.0,
            width: 8.0,
            height: 8.0,
        },
        ElementKind::Ellipse,
    );

    let (document, page_id) = document(vec![before, shape, after], Vec::new());
    let output = render(&document, page_id);

    assert_eq!(output.rendered_elements, 3);
    assert_eq!(output.skipped_elements, 0);
    assert!(output.svg.contains("points=\"10,20 50,20 30,50\""));
    assert!(output.svg.contains("transform=\"rotate(30 30 35)\""));
    assert!(
        output
            .svg
            .contains("stroke=\"#000000\" stroke-width=\"0.25\" fill=\"none\"")
    );

    let before_pos = output.svg.find(&before_id.0.to_string()).unwrap();
    let polygon_pos = output.svg.find(&polygon_id.0.to_string()).unwrap();
    let after_pos = output.svg.find(&after_id.0.to_string()).unwrap();
    assert!(before_pos < polygon_pos && polygon_pos < after_pos);
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive { element_id, .. } if *element_id == polygon_id
    )));
}

#[test]
fn reuses_shape_stroke_fill_gradient_alpha_and_system_palette_fallback() {
    let polygon_id = ElementId::new();
    let style_id = StyleId::new();
    let style = ElementStyle {
        id: style_id,
        stroke: Some(StrokeStyle {
            width_mm: 1.5,
            color: Color::SystemPalette { index: 7 },
        }),
        fill: Some(FillStyle {
            color: Color::Rgba {
                r: 10,
                g: 20,
                b: 30,
                a: 128,
            },
            gradient: Some(LinearGradient {
                end_color: Color::Rgba {
                    r: 200,
                    g: 210,
                    b: 220,
                    a: 64,
                },
                axis: GradientAxis::AlongY,
            }),
        }),
        text_color: None,
    };
    let shape = polygon(
        polygon_id,
        Some(style_id),
        vec![
            NormalizedPoint { x: 0.0, y: 0.0 },
            NormalizedPoint { x: 1.0, y: 0.0 },
            NormalizedPoint { x: 1.0, y: 1.0 },
            NormalizedPoint { x: 0.0, y: 1.0 },
        ],
    );
    let (document, page_id) = document(vec![shape], vec![style]);
    let output = render(&document, page_id);

    assert!(
        output
            .svg
            .contains("stroke=\"#808080\" stroke-width=\"1.5\"")
    );
    assert!(
        output
            .svg
            .contains("<linearGradient id=\"ddn-polygon-gradient-")
    );
    assert!(
        output
            .svg
            .contains("x1=\"0%\" y1=\"0%\" x2=\"0%\" y2=\"100%\"")
    );
    assert!(
        output
            .svg
            .contains("stop-color=\"#0a141e\" stop-opacity=\"0.502\"")
    );
    assert!(
        output
            .svg
            .contains("stop-color=\"#c8d2dc\" stop-opacity=\"0.251\"")
    );
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::SystemPaletteFallback { element_id, index: 7 }
            if *element_id == polygon_id
    )));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive { element_id, .. } if *element_id == polygon_id
    )));
}

#[test]
fn preserves_legacy_two_point_polygon_minimum() {
    let polygon_id = ElementId::new();
    let shape = polygon(
        polygon_id,
        None,
        vec![
            NormalizedPoint { x: 0.0, y: 0.5 },
            NormalizedPoint { x: 1.0, y: 0.5 },
        ],
    );
    let (document, page_id) = document(vec![shape], Vec::new());
    let output = render(&document, page_id);

    assert_eq!(output.rendered_elements, 1);
    assert_eq!(output.skipped_elements, 0);
    assert!(output.svg.contains("points=\"10,35 50,35\""));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::InvalidGeometry { element_id } if *element_id == polygon_id
    )));
}

#[test]
fn malformed_polygon_stays_skipped_with_explicit_geometry_diagnostic() {
    let polygon_id = ElementId::new();
    let shape = polygon(polygon_id, None, vec![NormalizedPoint { x: 0.0, y: 0.0 }]);
    let (document, page_id) = document(vec![shape], Vec::new());
    let output = render(&document, page_id);

    assert_eq!(output.rendered_elements, 0);
    assert_eq!(output.skipped_elements, 1);
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::InvalidGeometry { element_id } if *element_id == polygon_id
    )));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive { element_id, .. } if *element_id == polygon_id
    )));
}
