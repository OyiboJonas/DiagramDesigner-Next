use next_domain::{
    AnchorSet, Color, ConnectorLabelStyle, CurveKind, Document, DocumentDefaults, DocumentId,
    Element, ElementId, ElementKind, ElementStyle, Layer, LayerId, Page, PageId, Point, Rect,
    Scene, Size, StrokeStyle, StyleId,
};
use render_plan::{PreparedPage, RenderPlanOptions, build_page_plan};
use render_svg::{SvgRenderOptions, render_plan_to_svg};

fn fixture() -> (Document, PageId) {
    let page_id = PageId::new();
    let style_id = StyleId::new();
    let points = vec![
        Point { x: 100.0, y: 100.0 },
        Point { x: 110.0, y: 110.0 },
        Point { x: 120.0, y: 110.0 },
        Point { x: 130.0, y: 100.0 },
    ];
    let element = Element {
        id: ElementId::new(),
        name: "catmull-overshoot".to_owned(),
        bounds_mm: Rect {
            x: 100.0,
            y: 100.0,
            width: 30.0,
            height: 10.0,
        },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: Some(style_id),
        text: None,
        kind: ElementKind::Curve {
            curve_kind: CurveKind::CatmullRom,
            connector: None,
            control_points_mm: points,
        },
        import: None,
    };

    (
        Document {
            id: DocumentId::new(),
            name: "Curve prepared equivalence".to_owned(),
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
                    scene: Scene {
                        roots: vec![element.id],
                        elements: vec![element],
                    },
                }],
            }],
            styles: vec![ElementStyle {
                id: style_id,
                stroke: Some(StrokeStyle {
                    width_mm: 0.5,
                    color: Color::Rgba {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                }),
                fill: None,
                text_color: None,
            }],
            assets: Vec::new(),
            import: None,
        },
        page_id,
    )
}

#[test]
fn cold_and_prepared_plans_keep_visible_catmull_overshoot_and_render_identically() {
    let (document, page_id) = fixture();
    // The serialized/control-point rectangle ends at y=110. The public uniform
    // Catmull-Rom segment between the two middle points overshoots to about
    // y=111.25. This viewport intersects only that spline excursion.
    let options = RenderPlanOptions {
        viewport_mm: Some(Rect {
            x: 114.0,
            y: 110.4,
            width: 2.0,
            height: 0.7,
        }),
        cull_margin_mm: 0.0,
    };

    let cold = build_page_plan(&document, page_id, options).unwrap();
    let prepared_page = PreparedPage::build(&document, page_id).unwrap();
    let prepared = prepared_page.query(options).unwrap();

    assert_eq!(cold.items.len(), 1);
    assert_eq!(prepared.items.len(), 1);
    assert_eq!(cold.culled_elements, prepared.culled_elements);

    let render_options = SvgRenderOptions {
        view_box_mm: options.viewport_mm,
    };
    let cold_svg = render_plan_to_svg(&document, page_id, &cold, render_options).unwrap();
    let prepared_svg = render_plan_to_svg(&document, page_id, &prepared, render_options).unwrap();

    assert_eq!(cold_svg, prepared_svg);
    assert_eq!(cold_svg.rendered_elements, 1);
    assert_eq!(cold_svg.skipped_elements, 0);
    assert!(
        cold_svg
            .svg
            .contains("data-ddn-curve-kind=\"catmull-rom\"")
    );
}
