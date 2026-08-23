use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, Page, PageId, Rect, Scene, Size,
};
use render_plan::{PreparedPage, RenderPlanOptions, build_page_plan};
use render_svg::{SvgRenderOptions, render_plan_to_svg};

fn rectangle(id: ElementId, x: f64) -> Element {
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
        kind: ElementKind::Rectangle {
            corner_radius_mm: 1.5,
        },
        import: None,
    }
}

fn fixture() -> (Document, PageId) {
    let page_id = PageId::new();
    let inside_id = ElementId::new();
    let outside_id = ElementId::new();
    let elements = vec![rectangle(inside_id, 10.0), rectangle(outside_id, 160.0)];
    let roots = elements.iter().map(|element| element.id).collect();

    (
        Document {
            id: DocumentId::new(),
            name: "Prepared equivalence".to_owned(),
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
    )
}

#[test]
fn cold_and_prepared_viewport_plans_render_identical_svg() {
    let (document, page_id) = fixture();
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
    assert_eq!(cold.culled_elements, prepared.culled_elements);

    let render_options = SvgRenderOptions {
        view_box_mm: options.viewport_mm,
    };
    let cold_svg = render_plan_to_svg(&document, page_id, &cold, render_options).unwrap();
    let prepared_svg = render_plan_to_svg(&document, page_id, &prepared, render_options).unwrap();

    assert_eq!(cold_svg, prepared_svg);
}
