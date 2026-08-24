use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NormalizedPoint, Page, PageId, Rect, Scene, Size,
};
use render_plan::{PreparedPage, RenderPlanOptions, build_page_plan};
use render_svg::{SvgRenderOptions, render_plan_to_svg};

fn polygon(id: ElementId, x: f64) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm: Rect {
            x,
            y: 10.0,
            width: 20.0,
            height: 12.0,
        },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::Polygon {
            vertices: vec![
                NormalizedPoint { x: 0.0, y: 1.0 },
                NormalizedPoint { x: 0.5, y: 0.0 },
                NormalizedPoint { x: 1.0, y: 1.0 },
            ],
        },
        import: None,
    }
}

fn fixture() -> (Document, PageId, ElementId, ElementId) {
    let page_id = PageId::new();
    let inside_id = ElementId::new();
    let outside_id = ElementId::new();
    let elements = vec![polygon(inside_id, 10.0), polygon(outside_id, 160.0)];
    let roots = elements.iter().map(|element| element.id).collect();

    (
        Document {
            id: DocumentId::new(),
            name: "Polygon prepared equivalence".to_owned(),
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
                    scene: Scene { roots, elements },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        },
        page_id,
        inside_id,
        outside_id,
    )
}

#[test]
fn polygon_cold_and_prepared_viewport_plans_render_identical_svg() {
    let (document, page_id, inside_id, outside_id) = fixture();
    let options = RenderPlanOptions {
        viewport_mm: Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 80.0,
        }),
        cull_margin_mm: 0.0,
    };

    let cold = build_page_plan(&document, page_id, options).unwrap();
    let prepared_page = PreparedPage::build(&document, page_id).unwrap();
    let prepared = prepared_page.query(options).unwrap();

    assert_eq!(cold.items.len(), 1);
    assert_eq!(prepared.items.len(), 1);
    assert_eq!(cold.items[0].element.id, inside_id);
    assert_eq!(prepared.items[0].element.id, inside_id);
    assert_eq!(cold.culled_elements, prepared.culled_elements);

    let render_options = SvgRenderOptions {
        view_box_mm: options.viewport_mm,
    };
    let cold_svg = render_plan_to_svg(&document, page_id, &cold, render_options).unwrap();
    let prepared_svg = render_plan_to_svg(&document, page_id, &prepared, render_options).unwrap();

    assert_eq!(cold_svg, prepared_svg);
    assert_eq!(cold_svg.rendered_elements, 1);
    assert_eq!(cold_svg.skipped_elements, 0);
    assert!(cold_svg.svg.contains(&inside_id.0.to_string()));
    assert!(!cold_svg.svg.contains(&outside_id.0.to_string()));
}
