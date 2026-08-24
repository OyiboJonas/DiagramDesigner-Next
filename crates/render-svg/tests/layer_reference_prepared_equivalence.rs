use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, Page, PageId, Rect, Scene, Size,
};
use render_plan::{PreparedPage, RenderPlanOptions, build_page_plan};
use render_svg::{SvgRenderOptions, render_plan_to_svg};

fn layer(elements: Vec<Element>) -> Layer {
    let roots = elements.iter().map(|element| element.id).collect();
    Layer {
        id: LayerId::new(),
        name: String::new(),
        visible: true,
        locked: false,
        draw_color: None,
        scene: Scene { roots, elements },
    }
}

fn fixture() -> (Document, PageId, ElementId) {
    let target_page_id = PageId::new();
    let page_id = PageId::new();
    let target_id = ElementId::new();
    let reference_id = ElementId::new();
    let target = Element {
        id: target_id,
        name: String::new(),
        bounds_mm: Rect {
            x: 5.0,
            y: 5.0,
            width: 20.0,
            height: 10.0,
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
    };
    let reference = Element {
        id: reference_id,
        name: String::new(),
        bounds_mm: Rect {
            x: 100.0,
            y: 100.0,
            width: 100.0,
            height: 50.0,
        },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::LayerReference {
            relative_page_index: -1,
            layer_index: 0,
        },
        import: None,
    };

    (
        Document {
            id: DocumentId::new(),
            name: "Layer reference prepared equivalence".to_owned(),
            defaults: DocumentDefaults {
                font_family: "Arial".to_owned(),
                font_size_pt: 10.0,
                font_style_bits: 0,
                object_shadows: false,
                auto_line_break: true,
                connector_label_style: ConnectorLabelStyle::Transparent,
            },
            master_layers: Vec::new(),
            pages: vec![
                Page {
                    id: target_page_id,
                    name: "Target".to_owned(),
                    size_mm: Size {
                        width: 100.0,
                        height: 50.0,
                    },
                    layers: vec![layer(vec![target])],
                },
                Page {
                    id: page_id,
                    name: "Current".to_owned(),
                    size_mm: Size {
                        width: 300.0,
                        height: 200.0,
                    },
                    layers: vec![layer(vec![reference])],
                },
            ],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        },
        page_id,
        target_id,
    )
}

#[test]
fn cold_and_prepared_plans_render_visible_layer_reference_identically() {
    let (document, page_id, target_id) = fixture();
    let options = RenderPlanOptions {
        // The referenced rectangle maps from x=5..25 on a 100 mm source page to
        // x=105..125 inside the inherited-layer object. The viewport intersects
        // that materialized content while remaining a narrow partial-page query.
        viewport_mm: Some(Rect {
            x: 108.0,
            y: 108.0,
            width: 8.0,
            height: 6.0,
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
    assert!(cold_svg.svg.contains(&target_id.0.to_string()));
    assert!(cold_svg.svg.contains("data-ddn-layer-reference-page=\"0\""));
}
