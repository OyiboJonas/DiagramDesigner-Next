use std::error::Error;
use std::io::{self, Write};

use next_domain::{
    AnchorSet, Asset, AssetId, AssetPayload, Color, Connector, ConnectorLabelStyle, Document,
    DocumentDefaults, DocumentId, Element, ElementId, ElementKind, ElementStyle, Endpoint,
    FillStyle, GradientAxis, Layer, LayerId, LineStyle, LinearGradient, MarkerStyle, NextArtifact,
    NormalizedPoint, Page, PageId, Point, Rect, RichTextDocument, RichTextToken, Scene, Size,
    StrokeStyle, StyleId, TextBlock, TextHorizontalAlignment, TextLayout, TextStyle,
    TextVerticalAlignment,
};
use render_plan::{RenderPlanOptions, build_page_plan};
use render_svg::{SvgRenderOptions, render_plan_to_svg};

fn main() -> Result<(), Box<dyn Error>> {
    let (document, page_id) = fidelity_document();
    let validation = NextArtifact::document(document.clone()).validate();
    if !validation.is_valid() {
        return Err(format!(
            "fidelity fixture is not a valid next-domain document: {validation:?}"
        )
        .into());
    }

    let plan = build_page_plan(&document, page_id, RenderPlanOptions::default())?;
    for diagnostic in &plan.diagnostics {
        eprintln!("PLAN-DIAGNOSTIC {diagnostic:?}");
    }

    let rendered = render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default())?;
    for diagnostic in &rendered.diagnostics {
        eprintln!("SVG-DIAGNOSTIC {diagnostic:?}");
    }
    eprintln!(
        "FIDELITY-SUMMARY rendered={} skipped={} plan_diagnostics={} svg_diagnostics={}",
        rendered.rendered_elements,
        rendered.skipped_elements,
        plan.diagnostics.len(),
        rendered.diagnostics.len(),
    );

    io::stdout().write_all(rendered.svg.as_bytes())?;
    io::stdout().write_all(b"\n")?;
    Ok(())
}

fn fidelity_document() -> (Document, PageId) {
    // This fixed UUID is used only as a UUID-v5 namespace. Every fixture identity
    // therefore remains stable across machines and runs without adding a renderer
    // dependency on UUID generation or leaking test identity into production state.
    let namespace = "6d6d2140-4044-4e58-94f7-1b6e2fc8f6fb"
        .parse()
        .expect("valid ADR-019 fidelity UUID namespace");
    let page_id = PageId::v5(namespace, "adr-019-fidelity/page/1");

    let master_style_id = StyleId::v5(namespace, "adr-019-fidelity/style/master");
    let foreground_style_id = StyleId::v5(namespace, "adr-019-fidelity/style/foreground");
    let ellipse_style_id = StyleId::v5(namespace, "adr-019-fidelity/style/ellipse");
    let text_style_id = StyleId::v5(namespace, "adr-019-fidelity/style/text");
    let connector_style_id = StyleId::v5(namespace, "adr-019-fidelity/style/connector");

    let master_id = ElementId::v5(namespace, "adr-019-fidelity/element/master");
    let foreground_id = ElementId::v5(namespace, "adr-019-fidelity/element/foreground");
    let ellipse_id = ElementId::v5(namespace, "adr-019-fidelity/element/ellipse");
    let text_id = ElementId::v5(namespace, "adr-019-fidelity/element/text");
    let dotted_connector_id = ElementId::v5(namespace, "adr-019-fidelity/element/dotted");
    let dash_connector_id = ElementId::v5(namespace, "adr-019-fidelity/element/dash-dot");
    let marker_connector_id = ElementId::v5(namespace, "adr-019-fidelity/element/marker");
    let deferred_label_id = ElementId::v5(namespace, "adr-019-fidelity/element/diagnostic-label");
    let polygon_id = ElementId::v5(namespace, "adr-019-fidelity/element/polygon");
    let raster_image_id = ElementId::v5(namespace, "adr-019-fidelity/element/raster-image");
    let raster_asset_id = AssetId::v5(namespace, "adr-019-fidelity/asset/raster-image");
    let edge_left_id = ElementId::v5(namespace, "adr-019-fidelity/element/edge-left");
    let edge_right_id = ElementId::v5(namespace, "adr-019-fidelity/element/edge-right");
    let edge_top_id = ElementId::v5(namespace, "adr-019-fidelity/element/edge-top");
    let edge_bottom_id = ElementId::v5(namespace, "adr-019-fidelity/element/edge-bottom");

    let master = element(
        master_id,
        "MASTER — should stay behind local content",
        Rect {
            x: 18.0,
            y: 18.0,
            width: 112.0,
            height: 72.0,
        },
        0.0,
        Some(master_style_id),
        ElementKind::Rectangle {
            corner_radius_mm: 2.0,
        },
    );

    let foreground = element(
        foreground_id,
        "LOCAL — overlaps master",
        Rect {
            x: 58.0,
            y: 48.0,
            width: 122.0,
            height: 68.0,
        },
        12.0,
        Some(foreground_style_id),
        ElementKind::Rectangle {
            corner_radius_mm: 7.0,
        },
    );

    let ellipse = element(
        ellipse_id,
        "Rotated alpha ellipse",
        Rect {
            x: 196.0,
            y: 34.0,
            width: 68.0,
            height: 48.0,
        },
        -17.0,
        Some(ellipse_style_id),
        ElementKind::Ellipse,
    );

    let mut text = element(
        text_id,
        "Rich text and page fields",
        Rect {
            x: 22.0,
            y: 126.0,
            width: 250.0,
            height: 24.0,
        },
        0.0,
        Some(text_style_id),
        ElementKind::Text,
    );
    text.text = Some(TextBlock {
        content: RichTextDocument {
            tokens: vec![
                RichTextToken::Text {
                    text: "Fidelity ÄÖÜ • → <&> — ".to_owned(),
                    style: TextStyle {
                        bold: true,
                        font_size_pt: Some(13),
                        ..TextStyle::default()
                    },
                },
                RichTextToken::PageName {
                    style: TextStyle::default(),
                },
                RichTextToken::Text {
                    text: " — page ".to_owned(),
                    style: TextStyle::default(),
                },
                RichTextToken::PageNumber {
                    style: TextStyle::default(),
                },
                RichTextToken::Text {
                    text: "/".to_owned(),
                    style: TextStyle::default(),
                },
                RichTextToken::PageCount {
                    style: TextStyle::default(),
                },
            ],
            tail: None,
            diagnostics: Vec::new(),
        },
        layout: TextLayout {
            horizontal: TextHorizontalAlignment::Left,
            vertical: TextVerticalAlignment::Center,
            margin_mm: 1.0,
        },
    });

    let dotted_connector = connector_element(
        dotted_connector_id,
        "Dotted connector",
        Point { x: 28.0, y: 172.0 },
        Point { x: 126.0, y: 172.0 },
        LineStyle::Dotted1,
        MarkerStyle::None,
        Some(connector_style_id),
    );
    let dash_connector = connector_element(
        dash_connector_id,
        "Dash-dot connector",
        Point { x: 28.0, y: 190.0 },
        Point { x: 126.0, y: 190.0 },
        LineStyle::DashDot1,
        MarkerStyle::None,
        Some(connector_style_id),
    );
    let marker_connector = connector_element(
        marker_connector_id,
        "Deferred marker connector",
        Point { x: 154.0, y: 172.0 },
        Point { x: 258.0, y: 172.0 },
        LineStyle::Solid,
        MarkerStyle::Arrow1,
        Some(connector_style_id),
    );

    let mut deferred_label = element(
        deferred_label_id,
        "Expected diagnostic labels",
        Rect {
            x: 154.0,
            y: 180.0,
            width: 126.0,
            height: 24.0,
        },
        0.0,
        Some(text_style_id),
        ElementKind::Text,
    );
    deferred_label.text = Some(TextBlock {
        content: RichTextDocument {
            tokens: vec![RichTextToken::Text {
                text: "Arrow marker, polygon and raster image are rendered by the Phase-2 production facade"
                    .to_owned(),
                style: TextStyle {
                    font_size_pt: Some(8),
                    ..TextStyle::default()
                },
            }],
            tail: None,
            diagnostics: Vec::new(),
        },
        layout: TextLayout {
            horizontal: TextHorizontalAlignment::Left,
            vertical: TextVerticalAlignment::Top,
            margin_mm: 0.5,
        },
    });

    let polygon = element(
        polygon_id,
        "Rendered polygon sentinel",
        Rect {
            x: 252.0,
            y: 188.0,
            width: 24.0,
            height: 16.0,
        },
        0.0,
        Some(foreground_style_id),
        ElementKind::Polygon {
            vertices: vec![
                NormalizedPoint { x: 0.0, y: 1.0 },
                NormalizedPoint { x: 0.5, y: 0.0 },
                NormalizedPoint { x: 1.0, y: 1.0 },
            ],
        },
    );

    let raster_image = element(
        raster_image_id,
        "Rendered raster image sentinel",
        Rect {
            x: 214.0,
            y: 154.0,
            width: 28.0,
            height: 18.0,
        },
        0.0,
        None,
        ElementKind::Image {
            asset_id: raster_asset_id,
        },
    );

    // Four partially clipped, rotated rectangles make page-edge/culling mistakes
    // obvious in a manual review. Their document bounds intentionally cross the
    // page viewBox on every side; conservative rotated AABB planning must keep
    // the visible portions rather than dropping them at the edge.
    let edge_left = edge_sentinel(
        edge_left_id,
        "LEFT edge sentinel",
        Rect {
            x: -4.0,
            y: 92.0,
            width: 12.0,
            height: 22.0,
        },
        20.0,
        foreground_style_id,
    );
    let edge_right = edge_sentinel(
        edge_right_id,
        "RIGHT edge sentinel",
        Rect {
            x: 289.0,
            y: 92.0,
            width: 12.0,
            height: 22.0,
        },
        -20.0,
        foreground_style_id,
    );
    let edge_top = edge_sentinel(
        edge_top_id,
        "TOP edge sentinel",
        Rect {
            x: 142.0,
            y: -4.0,
            width: 18.0,
            height: 12.0,
        },
        28.0,
        foreground_style_id,
    );
    let edge_bottom = edge_sentinel(
        edge_bottom_id,
        "BOTTOM edge sentinel",
        Rect {
            x: 142.0,
            y: 202.0,
            width: 18.0,
            height: 12.0,
        },
        -28.0,
        foreground_style_id,
    );

    let master_layer = Layer {
        id: LayerId::v5(namespace, "adr-019-fidelity/layer/master"),
        name: "Master fidelity layer".to_owned(),
        visible: true,
        locked: false,
        draw_color: None,
        scene: Scene {
            roots: vec![master_id],
            elements: vec![master],
        },
    };

    let local_elements = vec![
        foreground,
        ellipse,
        text,
        dotted_connector,
        dash_connector,
        marker_connector,
        deferred_label,
        polygon,
        raster_image,
        edge_left,
        edge_right,
        edge_top,
        edge_bottom,
    ];
    let local_roots = local_elements.iter().map(|element| element.id).collect();
    let local_layer = Layer {
        id: LayerId::v5(namespace, "adr-019-fidelity/layer/local"),
        name: "Local fidelity layer".to_owned(),
        visible: true,
        locked: false,
        draw_color: None,
        scene: Scene {
            roots: local_roots,
            elements: local_elements,
        },
    };

    let document = Document {
        id: DocumentId::v5(namespace, "adr-019-fidelity/document"),
        name: "ADR-019 fidelity fixture".to_owned(),
        defaults: DocumentDefaults {
            font_family: "Segoe UI".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Transparent,
        },
        master_layers: vec![master_layer],
        pages: vec![Page {
            id: page_id,
            name: "Fidelity — Page 1".to_owned(),
            size_mm: Size {
                width: 297.0,
                height: 210.0,
            },
            layers: vec![local_layer],
        }],
        styles: vec![
            ElementStyle {
                id: master_style_id,
                stroke: Some(StrokeStyle {
                    width_mm: 0.7,
                    color: rgba(70, 78, 90, 255),
                }),
                fill: Some(FillStyle {
                    color: rgba(216, 220, 226, 255),
                    gradient: None,
                }),
                text_color: None,
            },
            ElementStyle {
                id: foreground_style_id,
                stroke: Some(StrokeStyle {
                    width_mm: 0.8,
                    color: rgba(27, 79, 114, 255),
                }),
                fill: Some(FillStyle {
                    color: rgba(103, 178, 219, 230),
                    gradient: Some(LinearGradient {
                        end_color: rgba(217, 126, 73, 230),
                        axis: GradientAxis::AlongX,
                    }),
                }),
                text_color: None,
            },
            ElementStyle {
                id: ellipse_style_id,
                stroke: Some(StrokeStyle {
                    width_mm: 0.6,
                    color: rgba(88, 61, 135, 255),
                }),
                fill: Some(FillStyle {
                    color: rgba(156, 123, 190, 150),
                    gradient: Some(LinearGradient {
                        end_color: rgba(95, 181, 161, 150),
                        axis: GradientAxis::AlongY,
                    }),
                }),
                text_color: None,
            },
            ElementStyle {
                id: text_style_id,
                stroke: None,
                fill: None,
                text_color: Some(rgba(24, 29, 36, 255)),
            },
            ElementStyle {
                id: connector_style_id,
                stroke: Some(StrokeStyle {
                    width_mm: 0.8,
                    color: rgba(39, 44, 52, 255),
                }),
                fill: None,
                text_color: None,
            },
        ],
        assets: vec![Asset {
            id: raster_asset_id,
            sha256: "3211a3f4ef985496bca12a5c1a89bd8d0bf92c22432d435815836fd293561a37".to_owned(),
            media_type: "application/vnd.diagramdesigner-next.raster".to_owned(),
            payload: AssetPayload::Raster {
                width: 2,
                height: 2,
                bits_per_pixel: 24,
                palette: None,
                pixels: vec![0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 255, 255],
                alpha: Some(vec![255, 192, 128, 64]),
                alpha_value: 192,
            },
        }],
        import: None,
    };

    (document, page_id)
}

fn element(
    id: ElementId,
    name: &str,
    bounds_mm: Rect,
    rotation_deg: f64,
    style_id: Option<StyleId>,
    kind: ElementKind,
) -> Element {
    Element {
        id,
        name: name.to_owned(),
        bounds_mm,
        rotation_deg,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id,
        text: None,
        kind,
        import: None,
    }
}

fn edge_sentinel(
    id: ElementId,
    name: &str,
    bounds_mm: Rect,
    rotation_deg: f64,
    style_id: StyleId,
) -> Element {
    element(
        id,
        name,
        bounds_mm,
        rotation_deg,
        Some(style_id),
        ElementKind::Rectangle {
            corner_radius_mm: 0.0,
        },
    )
}

fn connector_element(
    id: ElementId,
    name: &str,
    start: Point,
    end: Point,
    line_style: LineStyle,
    end_marker: MarkerStyle,
    style_id: Option<StyleId>,
) -> Element {
    let x = start.x.min(end.x);
    let y = start.y.min(end.y);
    let width = (end.x - start.x).abs().max(0.1);
    let height = (end.y - start.y).abs().max(0.1);
    element(
        id,
        name,
        Rect {
            x,
            y,
            width,
            height,
        },
        0.0,
        style_id,
        ElementKind::StraightConnector {
            connector: Connector {
                start: Endpoint {
                    position_mm: start,
                    connection: None,
                },
                end: Endpoint {
                    position_mm: end,
                    connection: None,
                },
                start_marker: MarkerStyle::None,
                end_marker,
                line_style,
                secondary_color: None,
            },
        },
    )
}

const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::Rgba { r, g, b, a }
}
