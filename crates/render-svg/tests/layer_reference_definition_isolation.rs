use next_domain::{
    AnchorSet, Color, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,
    ElementId, ElementKind, ElementStyle, FillStyle, GradientAxis, Layer, LayerId, LinearGradient,
    Page, PageId, Rect, Scene, Size, StrokeStyle, StyleId,
};
use render_plan::{RenderPlanOptions, build_page_plan};
use render_svg::{SvgRenderOptions, render_plan_to_svg};

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

fn rectangle(id: ElementId, bounds_mm: Rect, style_id: Option<StyleId>) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm,
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id,
        text: None,
        kind: ElementKind::Rectangle {
            corner_radius_mm: 0.0,
        },
        import: None,
    }
}

fn ellipse(id: ElementId, bounds_mm: Rect) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm,
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::Ellipse,
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

fn layer(elements: Vec<Element>, visible: bool) -> Layer {
    let roots = elements.iter().map(|element| element.id).collect();
    Layer {
        id: LayerId::new(),
        name: String::new(),
        visible,
        locked: false,
        draw_color: None,
        scene: Scene { roots, elements },
    }
}

fn page(id: PageId, layers: Vec<Layer>) -> Page {
    Page {
        id,
        name: String::new(),
        size_mm: Size {
            width: 100.0,
            height: 100.0,
        },
        layers,
    }
}

fn document(pages: Vec<Page>, styles: Vec<ElementStyle>) -> Document {
    Document {
        id: DocumentId::new(),
        name: "Layer reference definition isolation".to_owned(),
        defaults: defaults(),
        master_layers: Vec::new(),
        pages,
        styles,
        assets: Vec::new(),
        import: None,
    }
}

#[test]
fn repeated_references_namespace_svg_definition_ids_without_changing_element_ids() {
    let page0_id = PageId::new();
    let page1_id = PageId::new();
    let first_reference_id = ElementId::new();
    let second_reference_id = ElementId::new();
    let target_id = ElementId::new();
    let style_id = StyleId::new();
    let style = ElementStyle {
        id: style_id,
        stroke: Some(StrokeStyle {
            width_mm: 0.5,
            color: Color::Rgba {
                r: 10,
                g: 20,
                b: 30,
                a: 255,
            },
        }),
        fill: Some(FillStyle {
            color: Color::Rgba {
                r: 230,
                g: 220,
                b: 210,
                a: 255,
            },
            gradient: Some(LinearGradient {
                end_color: Color::Rgba {
                    r: 80,
                    g: 100,
                    b: 120,
                    a: 255,
                },
                axis: GradientAxis::AlongX,
            }),
        }),
        text_color: None,
    };

    let document = document(
        vec![
            page(
                page0_id,
                vec![layer(
                    vec![
                        layer_reference(
                            first_reference_id,
                            Rect {
                                x: 0.0,
                                y: 0.0,
                                width: 40.0,
                                height: 40.0,
                            },
                            1,
                            0,
                        ),
                        layer_reference(
                            second_reference_id,
                            Rect {
                                x: 50.0,
                                y: 50.0,
                                width: 40.0,
                                height: 40.0,
                            },
                            1,
                            0,
                        ),
                    ],
                    true,
                )],
            ),
            page(
                page1_id,
                vec![layer(
                    vec![rectangle(
                        target_id,
                        Rect {
                            x: 10.0,
                            y: 10.0,
                            width: 50.0,
                            height: 40.0,
                        },
                        Some(style_id),
                    )],
                    true,
                )],
            ),
        ],
        vec![style],
    );

    let plan = build_page_plan(&document, page0_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page0_id, &plan, SvgRenderOptions::default()).unwrap();

    let base_gradient = format!("gradient-{}", target_id.0);
    let first_gradient = format!(
        "ddn-layer-ref-{}-{base_gradient}",
        first_reference_id.0
    );
    let second_gradient = format!(
        "ddn-layer-ref-{}-{base_gradient}",
        second_reference_id.0
    );

    assert!(output.svg.contains(&format!("id=\"{first_gradient}\"")));
    assert!(output.svg.contains(&format!("url(#{first_gradient})")));
    assert!(output.svg.contains(&format!("id=\"{second_gradient}\"")));
    assert!(output.svg.contains(&format!("url(#{second_gradient})")));
    assert!(!output.svg.contains(&format!(" id=\"{base_gradient}\"")));
    assert_eq!(
        output
            .svg
            .matches(&format!("data-element-id=\"{}\"", target_id.0))
            .count(),
        2
    );
}

#[test]
fn direct_target_layer_render_ignores_page_visibility_and_excludes_other_layers() {
    let page0_id = PageId::new();
    let page1_id = PageId::new();
    let reference_id = ElementId::new();
    let unwanted_id = ElementId::new();
    let wanted_id = ElementId::new();

    let document = document(
        vec![
            page(
                page0_id,
                vec![layer(
                    vec![layer_reference(
                        reference_id,
                        Rect {
                            x: 10.0,
                            y: 20.0,
                            width: 60.0,
                            height: 50.0,
                        },
                        1,
                        1,
                    )],
                    true,
                )],
            ),
            page(
                page1_id,
                vec![
                    layer(
                        vec![ellipse(
                            unwanted_id,
                            Rect {
                                x: 5.0,
                                y: 5.0,
                                width: 10.0,
                                height: 10.0,
                            },
                        )],
                        true,
                    ),
                    layer(
                        vec![rectangle(
                            wanted_id,
                            Rect {
                                x: 20.0,
                                y: 25.0,
                                width: 30.0,
                                height: 20.0,
                            },
                            None,
                        )],
                        false,
                    ),
                ],
            ),
        ],
        Vec::new(),
    );

    let plan = build_page_plan(&document, page0_id, RenderPlanOptions::default()).unwrap();
    let output =
        render_plan_to_svg(&document, page0_id, &plan, SvgRenderOptions::default()).unwrap();

    assert!(output.svg.contains(&wanted_id.0.to_string()));
    assert!(!output.svg.contains(&unwanted_id.0.to_string()));
}
