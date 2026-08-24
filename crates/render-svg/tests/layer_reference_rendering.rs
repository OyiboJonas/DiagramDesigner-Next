use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, Page, PageId, Rect, Scene, Size,
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

fn rectangle(id: ElementId, x: f64, y: f64, width: f64, height: f64) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm: Rect {
            x,
            y,
            width,
            height,
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

fn layer_reference(
    id: ElementId,
    bounds_mm: Rect,
    relative_page_index: i32,
    layer_index: i32,
) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm,
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::LayerReference {
            relative_page_index,
            layer_index,
        },
        import: None,
    }
}

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

fn page(id: PageId, width: f64, height: f64, layers: Vec<Layer>) -> Page {
    Page {
        id,
        name: String::new(),
        size_mm: Size { width, height },
        layers,
    }
}

fn document(pages: Vec<Page>) -> Document {
    Document {
        id: DocumentId::new(),
        name: "Layer reference regression".to_owned(),
        defaults: defaults(),
        master_layers: Vec::new(),
        pages,
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    }
}

#[test]
fn renders_backward_and_forward_layer_references_with_page_scaling() {
    let page0_id = PageId::new();
    let page1_id = PageId::new();
    let page2_id = PageId::new();
    let backward_target_id = ElementId::new();
    let forward_target_id = ElementId::new();
    let backward_reference_id = ElementId::new();
    let middle_id = ElementId::new();
    let forward_reference_id = ElementId::new();
    let invalid_reference_id = ElementId::new();

    let backward_reference = layer_reference(
        backward_reference_id,
        Rect {
            x: 10.0,
            y: 20.0,
            width: 200.0,
            height: 100.0,
        },
        -1,
        0,
    );
    let forward_reference = layer_reference(
        forward_reference_id,
        Rect {
            x: 30.0,
            y: 140.0,
            width: 160.0,
            height: 80.0,
        },
        1,
        0,
    );
    let invalid_reference = layer_reference(
        invalid_reference_id,
        Rect {
            x: 5.0,
            y: 240.0,
            width: 20.0,
            height: 20.0,
        },
        99,
        0,
    );

    let document = document(vec![
        page(
            page0_id,
            100.0,
            50.0,
            vec![layer(vec![rectangle(
                backward_target_id,
                20.0,
                10.0,
                30.0,
                15.0,
            )])],
        ),
        page(
            page1_id,
            210.0,
            297.0,
            vec![layer(vec![
                backward_reference,
                rectangle(middle_id, 80.0, 110.0, 20.0, 20.0),
                forward_reference,
                invalid_reference,
            ])],
        ),
        page(
            page2_id,
            80.0,
            40.0,
            vec![layer(vec![rectangle(
                forward_target_id,
                5.0,
                6.0,
                12.0,
                8.0,
            )])],
        ),
    ]);

    let plan = build_page_plan(&document, page1_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page1_id, &plan, SvgRenderOptions::default()).unwrap();

    assert_eq!(output.rendered_elements, 3);
    assert_eq!(output.skipped_elements, 1);
    assert!(output.svg.contains(&format!(
        "data-element-id=\"{}\" data-ddn-layer-reference-page=\"0\" data-ddn-layer-reference-layer=\"0\"",
        backward_reference_id.0
    )));
    assert!(output.svg.contains(
        "<svg x=\"10\" y=\"20\" width=\"200\" height=\"100\" viewBox=\"0 0 100 50\" preserveAspectRatio=\"none\" overflow=\"visible\">"
    ));
    assert!(output.svg.contains(&backward_target_id.0.to_string()));

    assert!(output.svg.contains(&format!(
        "data-element-id=\"{}\" data-ddn-layer-reference-page=\"2\" data-ddn-layer-reference-layer=\"0\"",
        forward_reference_id.0
    )));
    assert!(output.svg.contains(
        "<svg x=\"30\" y=\"140\" width=\"160\" height=\"80\" viewBox=\"0 0 80 40\" preserveAspectRatio=\"none\" overflow=\"visible\">"
    ));
    assert!(output.svg.contains(&forward_target_id.0.to_string()));

    let backward_pos = output
        .svg
        .find(&backward_reference_id.0.to_string())
        .unwrap();
    let middle_pos = output.svg.find(&middle_id.0.to_string()).unwrap();
    let forward_pos = output
        .svg
        .find(&forward_reference_id.0.to_string())
        .unwrap();
    assert!(backward_pos < middle_pos && middle_pos < forward_pos);

    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive {
            element_id,
            family: RenderPrimitiveFamily::LayerReference,
        } if *element_id == backward_reference_id || *element_id == forward_reference_id
    )));
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive {
            element_id,
            family: RenderPrimitiveFamily::LayerReference,
        } if *element_id == invalid_reference_id
    )));
}

#[test]
fn nested_layer_references_resolve_relative_to_each_target_page() {
    let page0_id = PageId::new();
    let page1_id = PageId::new();
    let page2_id = PageId::new();
    let outer_id = ElementId::new();
    let inner_id = ElementId::new();
    let target_id = ElementId::new();

    let document = document(vec![
        page(
            page0_id,
            100.0,
            100.0,
            vec![layer(vec![layer_reference(
                outer_id,
                Rect {
                    x: 10.0,
                    y: 10.0,
                    width: 80.0,
                    height: 80.0,
                },
                1,
                0,
            )])],
        ),
        page(
            page1_id,
            50.0,
            50.0,
            vec![layer(vec![layer_reference(
                inner_id,
                Rect {
                    x: 5.0,
                    y: 5.0,
                    width: 40.0,
                    height: 40.0,
                },
                1,
                0,
            )])],
        ),
        page(
            page2_id,
            25.0,
            25.0,
            vec![layer(vec![rectangle(target_id, 2.0, 3.0, 4.0, 5.0)])],
        ),
    ]);

    let plan = build_page_plan(&document, page0_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page0_id, &plan, SvgRenderOptions::default()).unwrap();

    assert_eq!(output.rendered_elements, 1);
    assert_eq!(output.skipped_elements, 0);
    assert!(output.svg.contains(&outer_id.0.to_string()));
    assert!(output.svg.contains(&inner_id.0.to_string()));
    assert!(output.svg.contains(&target_id.0.to_string()));
    assert!(output.svg.contains("data-ddn-layer-reference-page=\"1\""));
    assert!(output.svg.contains("data-ddn-layer-reference-page=\"2\""));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive {
            family: RenderPrimitiveFamily::LayerReference,
            ..
        }
    )));
}

#[test]
fn recursive_layer_reference_is_suppressed_without_fallback_rendering() {
    let page_id = PageId::new();
    let reference_id = ElementId::new();
    let document = document(vec![page(
        page_id,
        100.0,
        100.0,
        vec![layer(vec![layer_reference(
            reference_id,
            Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            0,
            0,
        )])],
    )]);

    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert_eq!(output.rendered_elements, 1);
    assert_eq!(output.skipped_elements, 0);
    assert_eq!(
        output
            .svg
            .matches(&format!("data-element-id=\"{}\"", reference_id.0))
            .count(),
        1
    );
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive {
            element_id,
            family: RenderPrimitiveFamily::LayerReference,
        } if *element_id == reference_id
    )));
}
