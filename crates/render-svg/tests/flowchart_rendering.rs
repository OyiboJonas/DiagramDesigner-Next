use next_domain::{
    AnchorSet, Color, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,
    ElementId, ElementKind, ElementStyle, FillStyle, Layer, LayerId, Page, Rect, Scene, Size,
    StrokeStyle, StyleId,
};
use render_plan::{RenderPlanOptions, RenderPrimitiveFamily, build_page_plan};
use render_svg::{SvgDiagnostic, SvgRenderOptions, render_plan_to_svg};

const FLOWCHART_PREFIX: &str = "builtin:diagramdesigner-flowchart/";

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

fn flowchart(id: ElementId, style_id: StyleId, code: i32, x: f64, y: f64) -> Element {
    Element {
        id,
        name: format!("flowchart-{code}"),
        bounds_mm: Rect {
            x,
            y,
            width: 24.0,
            height: 16.0,
        },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: Some(style_id),
        text: None,
        kind: ElementKind::Flowchart {
            shape_key: format!("{FLOWCHART_PREFIX}{code}"),
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
            width: 10.0,
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
    }
}

fn document(elements: Vec<Element>, style: ElementStyle) -> (Document, next_domain::PageId) {
    let page_id = next_domain::PageId::new();
    let roots = elements.iter().map(|element| element.id).collect();
    (
        Document {
            id: DocumentId::new(),
            name: "Flowchart regression".to_owned(),
            defaults: defaults(),
            master_layers: Vec::new(),
            pages: vec![Page {
                id: page_id,
                name: "Page".to_owned(),
                size_mm: Size {
                    width: 240.0,
                    height: 240.0,
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

fn style() -> ElementStyle {
    ElementStyle {
        id: StyleId::new(),
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
                r: 240,
                g: 230,
                b: 220,
                a: 255,
            },
            gradient: None,
        }),
        text_color: None,
    }
}

#[test]
fn renders_all_eight_public_legacy_flowchart_kinds() {
    let style = style();
    let codes = [0x11, 0x21, 0x22, 0x23, 0x31, 0x32, 0x41, 0x51];
    let elements = codes
        .iter()
        .enumerate()
        .map(|(index, code)| {
            flowchart(
                ElementId::new(),
                style.id,
                *code,
                20.0 + (index % 2) as f64 * 80.0,
                15.0 + (index / 2) as f64 * 45.0,
            )
        })
        .collect();
    let (document, page_id) = document(elements, style);
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output = render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert_eq!(output.rendered_elements, 8);
    assert_eq!(output.skipped_elements, 0);
    for key in [
        "side-bars",
        "rounded-1",
        "rounded-2",
        "rounded-3",
        "slant-right",
        "slant-left",
        "odd-rounded-1",
        "odd-rounded-2",
    ] {
        assert!(
            output
                .svg
                .contains(&format!("data-ddn-flowchart-kind=\"{key}\"")),
            "missing rendered flowchart kind {key}"
        );
    }
    assert!(output.svg.contains("<line"), "side-bars must emit the two inner bars");
    assert!(output.svg.contains("rx=\"8\" ry=\"8\""));
    assert!(output.svg.contains("rx=\"4\" ry=\"4\""));
    assert!(output.svg.contains("rx=\"2\" ry=\"2\""));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive {
            family: RenderPrimitiveFamily::Flowchart,
            ..
        }
    )));
}

#[test]
fn unknown_flowchart_code_remains_explicitly_unsupported() {
    let style = style();
    let element_id = ElementId::new();
    let (document, page_id) = document(
        vec![flowchart(element_id, style.id, 0x99, 20.0, 20.0)],
        style,
    );
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output = render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    assert_eq!(output.rendered_elements, 0);
    assert_eq!(output.skipped_elements, 1);
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive {
            element_id: diagnostic_id,
            family: RenderPrimitiveFamily::Flowchart,
        } if *diagnostic_id == element_id
    )));
    assert!(!output.svg.contains("data-ddn-flowchart-kind="));
}

#[test]
fn flowchart_insertion_preserves_render_plan_z_order() {
    let style = style();
    let before_id = ElementId::new();
    let flowchart_id = ElementId::new();
    let after_id = ElementId::new();
    let (document, page_id) = document(
        vec![
            rectangle(before_id, 5.0),
            flowchart(flowchart_id, style.id, 0x31, 40.0, 10.0),
            rectangle(after_id, 90.0),
        ],
        style,
    );
    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
    let output = render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

    let before_pos = output.svg.find(&before_id.0.to_string()).unwrap();
    let flowchart_pos = output.svg.find(&flowchart_id.0.to_string()).unwrap();
    let after_pos = output.svg.find(&after_id.0.to_string()).unwrap();
    assert!(before_pos < flowchart_pos && flowchart_pos < after_pos);
}
