use next_domain::{
    AnchorSet, Color, Connector, ConnectorLabelStyle, CurveKind, Document, DocumentDefaults,
    DocumentId, Element, ElementId, ElementKind, ElementStyle, Endpoint, Layer, LayerId, LineStyle,
    MarkerStyle, Page, Point, Rect, Scene, Size, StrokeStyle, StyleId,
};
use render_plan::{RenderPlanOptions, RenderPrimitiveFamily, build_page_plan};
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

fn style() -> ElementStyle {
    ElementStyle {
        id: StyleId::new(),
        stroke: Some(StrokeStyle {
            width_mm: 0.8,
            color: Color::Rgba {
                r: 18,
                g: 52,
                b: 86,
                a: 255,
            },
        }),
        fill: None,
        text_color: None,
    }
}

fn curve(
    id: ElementId,
    style_id: StyleId,
    kind: CurveKind,
    points: Vec<Point>,
    connector: Option<Connector>,
) -> Element {
    let min_x = points.iter().map(|point| point.x).fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points.iter().map(|point| point.y).fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    Element {
        id,
        name: format!("curve-{kind:?}"),
        bounds_mm: Rect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: Some(style_id),
        text: None,
        kind: ElementKind::Curve {
            curve_kind: kind,
            connector,
            control_points_mm: points,
        },
        import: None,
    }
}

fn rectangle(id: ElementId, x: f64) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm: Rect {
            x,
            y: 10.0,
            width: 8.0,
            height: 8.0,
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

fn connector(points: &[Point], start_marker: MarkerStyle, end_marker: MarkerStyle) -> Connector {
    Connector {
        start: Endpoint {
            position_mm: points[0],
            connection: None,
        },
        end: Endpoint {
            position_mm: points[points.len() - 1],
            connection: None,
        },
        start_marker,
        end_marker,
        line_style: LineStyle::Outline,
        secondary_color: Some(Color::Rgba {
            r: 240,
            g: 230,
            b: 220,
            a: 255,
        }),
    }
}

fn document(elements: Vec<Element>, style: ElementStyle) -> (Document, next_domain::PageId) {
    let page_id = next_domain::PageId::new();
    let roots = elements.iter().map(|element| element.id).collect();
    (
        Document {
            id: DocumentId::new(),
            name: "Curve regression".to_owned(),
            defaults: defaults(),
            master_layers: Vec::new(),
            pages: vec![Page {
                id: page_id,
                name: "Page".to_owned(),
                size_mm: Size {
                    width: 260.0,
                    height: 220.0,
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
            styles: vec![style],
            assets: Vec::new(),
            import: None,
        },
        page_id,
    )
}

#[test]
fn renders_all_four_public_curve_families_and_retires_curve_diagnostics() {
    let style = style();
    let families = [
        (CurveKind::CatmullRom, "catmull-rom"),
        (CurveKind::Legacy, "legacy"),
        (CurveKind::Bezier, "bezier"),
        (CurveKind::LineSegments, "line-segments"),
    ];
    let elements = families
        .iter()
        .enumerate()
        .map(|(index, (kind, _))| {
            let y = 20.0 + index as f64 * 40.0;
            curve(
                ElementId::new(),
                style.id,
                *kind,
                vec![
                    Point { x: 20.0, y },
                    Point {
                        x: 45.0,
                        y: y + 18.0,
                    },
                    Point {
                        x: 70.0,
                        y: y - 10.0,
                    },
                    Point { x: 95.0, y },
                ],
                None,
            )
        })
        .collect();
    let (document, page_id) = document(elements, style);
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert_eq!(output.rendered_elements, 4);
    assert_eq!(output.skipped_elements, 0);
    for (_, key) in families {
        assert!(
            output
                .svg
                .contains(&format!("data-ddn-curve-kind=\"{key}\"")),
            "missing rendered curve family {key}"
        );
    }
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive {
            family: RenderPrimitiveFamily::Curve,
            ..
        }
    )));
}

#[test]
fn bezier_repeats_the_last_point_until_the_public_polybezier_count_is_valid() {
    let style = style();
    let id = ElementId::new();
    let (document, page_id) = document(
        vec![curve(
            id,
            style.id,
            CurveKind::Bezier,
            vec![
                Point { x: 10.0, y: 10.0 },
                Point { x: 20.0, y: 30.0 },
                Point { x: 30.0, y: 30.0 },
                Point { x: 40.0, y: 10.0 },
                Point { x: 50.0, y: 20.0 },
            ],
            None,
        )],
        style,
    );
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert!(output.svg.contains(
        "d=\"M 10 10 C 20 30,30 30,40 10 C 50 20,50 20,50 20\""
    ));
}

#[test]
fn two_point_catmull_and_legacy_curves_use_the_public_straight_line_fallback() {
    let style = style();
    let catmull_id = ElementId::new();
    let legacy_id = ElementId::new();
    let points = vec![Point { x: 10.0, y: 15.0 }, Point { x: 60.0, y: 35.0 }];
    let (document, page_id) = document(
        vec![
            curve(
                catmull_id,
                style.id,
                CurveKind::CatmullRom,
                points.clone(),
                None,
            ),
            curve(
                legacy_id,
                style.id,
                CurveKind::Legacy,
                points,
                None,
            ),
        ],
        style,
    );
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert_eq!(output.svg.matches("d=\"M 10 15 L 60 35\"").count(), 2);
}

#[test]
fn curve_markers_use_curve_direction_points_and_outline_secondary_paint() {
    let style = style();
    let id = ElementId::new();
    let points = vec![
        Point { x: 20.0, y: 20.0 },
        Point { x: 35.0, y: 45.0 },
        Point { x: 65.0, y: 45.0 },
        Point { x: 80.0, y: 20.0 },
    ];
    let curve_connector = connector(&points, MarkerStyle::Arrow2, MarkerStyle::Diamond);
    let (document, page_id) = document(
        vec![curve(
            id,
            style.id,
            CurveKind::LineSegments,
            points,
            Some(curve_connector),
        )],
        style,
    );
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert!(output.svg.contains("data-ddn-curve-marker-target=\"start\" x1=\"20\" y1=\"20\" x2=\"35\" y2=\"45\""));
    assert!(output.svg.contains("data-ddn-curve-marker-target=\"end\" x1=\"65\" y1=\"45\" x2=\"80\" y2=\"20\""));
    assert!(output.svg.contains("data-ddn-marker-style=\"arrow2\""));
    assert!(output.svg.contains("data-ddn-marker-style=\"diamond\""));
    assert!(output.svg.contains("fill=\"#f0e6dc\""));
    assert!(!output
        .diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic, SvgDiagnostic::ConnectorMarkerDeferred { .. })));
    // TCurveLineObject.Draw forces a solid pen for the curve body. The stored
    // connector line style influences marker painting, not the path itself.
    assert!(!output.svg.contains("stroke-dasharray="));
}

#[test]
fn custom_curve_marker_stays_explicitly_deferred() {
    let style = style();
    let id = ElementId::new();
    let points = vec![Point { x: 20.0, y: 20.0 }, Point { x: 80.0, y: 20.0 }];
    let mut curve_connector = connector(&points, MarkerStyle::Custom(0x9911), MarkerStyle::None);
    curve_connector.line_style = LineStyle::Solid;
    let (document, page_id) = document(
        vec![curve(
            id,
            style.id,
            CurveKind::LineSegments,
            points,
            Some(curve_connector),
        )],
        style,
    );
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert_eq!(output.rendered_elements, 1);
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::ConnectorMarkerDeferred {
            element_id,
            marker: MarkerStyle::Custom(0x9911),
        } if *element_id == id
    )));
}

#[test]
fn invalid_curve_geometry_is_typed_as_invalid_not_unsupported() {
    let style = style();
    let id = ElementId::new();
    let (document, page_id) = document(
        vec![curve(
            id,
            style.id,
            CurveKind::Legacy,
            vec![Point { x: 20.0, y: 20.0 }],
            None,
        )],
        style,
    );
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert_eq!(output.rendered_elements, 0);
    assert_eq!(output.skipped_elements, 1);
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::InvalidGeometry { element_id } if *element_id == id
    )));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive {
            element_id,
            family: RenderPrimitiveFamily::Curve,
        } if *element_id == id
    )));
}

#[test]
fn curve_insertion_preserves_render_plan_z_order() {
    let style = style();
    let before_id = ElementId::new();
    let curve_id = ElementId::new();
    let after_id = ElementId::new();
    let (document, page_id) = document(
        vec![
            rectangle(before_id, 5.0),
            curve(
                curve_id,
                style.id,
                CurveKind::LineSegments,
                vec![Point { x: 40.0, y: 20.0 }, Point { x: 70.0, y: 30.0 }],
                None,
            ),
            rectangle(after_id, 100.0),
        ],
        style,
    );
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    let before_pos = output.svg.find(&before_id.0.to_string()).unwrap();
    let curve_pos = output.svg.find(&curve_id.0.to_string()).unwrap();
    let after_pos = output.svg.find(&after_id.0.to_string()).unwrap();
    assert!(before_pos < curve_pos && curve_pos < after_pos);
}
