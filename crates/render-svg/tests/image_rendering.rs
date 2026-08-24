use std::io::Read as _;

use flate2::read::ZlibDecoder;
use next_domain::{
    AnchorSet, Asset, AssetId, AssetPayload, ConnectorLabelStyle, Document, DocumentDefaults,
    DocumentId, Element, ElementId, ElementKind, Layer, LayerId, NormalizedPoint, Page, Rect, Scene,
    Size,
};
use render_plan::{RenderPlanOptions, build_page_plan};
use render_svg::{
    RasterAssetIssue, SvgDiagnostic, SvgRenderOptions, SvgRenderOutput, render_plan_to_svg,
};

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

fn element(id: ElementId, x: f64, kind: ElementKind) -> Element {
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
        kind,
        import: None,
    }
}

fn image(id: ElementId, asset_id: AssetId, x: f64) -> Element {
    element(id, x, ElementKind::Image { asset_id })
}

fn document(elements: Vec<Element>, assets: Vec<Asset>) -> (Document, next_domain::PageId) {
    let page_id = next_domain::PageId::new();
    let roots = elements.iter().map(|element| element.id).collect();
    (
        Document {
            id: DocumentId::new(),
            name: "Image regression".to_owned(),
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
            styles: Vec::new(),
            assets,
            import: None,
        },
        page_id,
    )
}

fn raster_asset(
    id: AssetId,
    width: i32,
    height: i32,
    bits_per_pixel: u8,
    palette: Option<Vec<u8>>,
    pixels: Vec<u8>,
    alpha: Option<Vec<u8>>,
    alpha_value: u8,
) -> Asset {
    Asset {
        id,
        sha256: "synthetic-test-asset".to_owned(),
        media_type: "application/vnd.diagramdesigner-next.raster".to_owned(),
        payload: AssetPayload::Raster {
            width,
            height,
            bits_per_pixel,
            palette,
            pixels,
            alpha,
            alpha_value,
        },
    }
}

fn render(document: &Document, page_id: next_domain::PageId) -> SvgRenderOutput {
    let plan = build_page_plan(document, page_id, RenderPlanOptions::default()).unwrap();
    render_plan_to_svg(document, page_id, &plan, SvgRenderOptions::default()).unwrap()
}

#[test]
fn renders_24_bit_bgr_pixels_with_global_alpha() {
    let asset_id = AssetId::new();
    let image_id = ElementId::new();
    let asset = raster_asset(
        asset_id,
        2,
        1,
        24,
        None,
        vec![0, 0, 255, 0, 255, 0],
        None,
        128,
    );
    let (document, page_id) = document(vec![image(image_id, asset_id, 10.0)], vec![asset]);
    let output = render(&document, page_id);

    assert_eq!(output.rendered_elements, 1);
    assert_eq!(output.skipped_elements, 0);
    assert!(output.svg.contains("<image"));
    assert!(output.svg.contains("preserveAspectRatio=\"none\""));
    assert!(output.svg.contains("data-ddn-raster-bpp=\"24\""));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive { element_id, .. } if *element_id == image_id
    )));

    let (width, height, rgba) = decode_embedded_png(&output.svg);
    assert_eq!((width, height), (2, 1));
    assert_eq!(rgba, vec![255, 0, 0, 128, 0, 255, 0, 128]);
}

#[test]
fn renders_8_bit_bgr_palette_with_per_pixel_and_global_alpha() {
    let asset_id = AssetId::new();
    let image_id = ElementId::new();
    let mut palette = vec![0u8; 256 * 3];
    palette[3..6].copy_from_slice(&[3, 2, 1]);
    palette[6..9].copy_from_slice(&[30, 20, 10]);
    let asset = raster_asset(
        asset_id,
        2,
        1,
        8,
        Some(palette),
        vec![1, 2],
        Some(vec![255, 64]),
        128,
    );
    let (document, page_id) = document(vec![image(image_id, asset_id, 10.0)], vec![asset]);
    let output = render(&document, page_id);

    let (width, height, rgba) = decode_embedded_png(&output.svg);
    assert_eq!((width, height), (2, 1));
    assert_eq!(rgba, vec![1, 2, 3, 128, 10, 20, 30, 32]);
    assert!(output.diagnostics.is_empty());
}

#[test]
fn missing_binary_and_malformed_assets_keep_images_skipped_with_typed_diagnostics() {
    let missing_asset_id = AssetId::new();
    let binary_asset_id = AssetId::new();
    let malformed_asset_id = AssetId::new();
    let missing_image_id = ElementId::new();
    let binary_image_id = ElementId::new();
    let malformed_image_id = ElementId::new();

    let binary = Asset {
        id: binary_asset_id,
        sha256: "binary".to_owned(),
        media_type: "application/octet-stream".to_owned(),
        payload: AssetPayload::Binary {
            bytes: vec![1, 2, 3],
        },
    };
    let malformed = raster_asset(
        malformed_asset_id,
        2,
        1,
        24,
        None,
        vec![0, 1, 2],
        None,
        255,
    );
    let (document, page_id) = document(
        vec![
            image(missing_image_id, missing_asset_id, 10.0),
            image(binary_image_id, binary_asset_id, 40.0),
            image(malformed_image_id, malformed_asset_id, 70.0),
        ],
        vec![binary, malformed],
    );
    let output = render(&document, page_id);

    assert_eq!(output.rendered_elements, 0);
    assert_eq!(output.skipped_elements, 3);
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::MissingAsset { element_id, asset_id }
            if *element_id == missing_image_id && *asset_id == missing_asset_id
    )));
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedAssetPayload { element_id, asset_id }
            if *element_id == binary_image_id && *asset_id == binary_asset_id
    )));
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::InvalidRasterAsset {
            element_id,
            asset_id,
            issue: RasterAssetIssue::InvalidPixelLength {
                expected: 6,
                actual: 3,
            },
        } if *element_id == malformed_image_id && *asset_id == malformed_asset_id
    )));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive { element_id, .. }
            if [missing_image_id, binary_image_id, malformed_image_id].contains(element_id)
    )));
}

#[test]
fn preserves_plan_order_across_image_polygon_and_core_primitive() {
    let asset_id = AssetId::new();
    let image_id = ElementId::new();
    let polygon_id = ElementId::new();
    let rectangle_id = ElementId::new();
    let asset = raster_asset(
        asset_id,
        1,
        1,
        24,
        None,
        vec![30, 20, 10],
        None,
        255,
    );
    let polygon = element(
        polygon_id,
        40.0,
        ElementKind::Polygon {
            vertices: vec![
                NormalizedPoint { x: 0.0, y: 0.0 },
                NormalizedPoint { x: 1.0, y: 0.0 },
                NormalizedPoint { x: 0.5, y: 1.0 },
            ],
        },
    );
    let rectangle = element(
        rectangle_id,
        70.0,
        ElementKind::Rectangle {
            corner_radius_mm: 0.0,
        },
    );
    let (document, page_id) = document(
        vec![image(image_id, asset_id, 10.0), polygon, rectangle],
        vec![asset],
    );
    let output = render(&document, page_id);

    let image_pos = output.svg.find(&image_id.0.to_string()).unwrap();
    let polygon_pos = output.svg.find(&polygon_id.0.to_string()).unwrap();
    let rectangle_pos = output.svg.find(&rectangle_id.0.to_string()).unwrap();
    assert!(image_pos < polygon_pos && polygon_pos < rectangle_pos);
}

fn decode_embedded_png(svg: &str) -> (u32, u32, Vec<u8>) {
    let prefix = "href=\"data:image/png;base64,";
    let start = svg.find(prefix).expect("embedded PNG data URL") + prefix.len();
    let end = svg[start..].find('"').expect("data URL closing quote") + start;
    let png = base64_decode(&svg[start..end]);
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");

    let mut offset = 8usize;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut idat = Vec::new();
    while offset < png.len() {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        let kind = &png[offset + 4..offset + 8];
        let data_start = offset + 8;
        let data_end = data_start + length;
        let data = &png[data_start..data_end];
        if kind == b"IHDR" {
            width = u32::from_be_bytes(data[0..4].try_into().unwrap());
            height = u32::from_be_bytes(data[4..8].try_into().unwrap());
            assert_eq!(&data[8..13], &[8, 6, 0, 0, 0]);
        } else if kind == b"IDAT" {
            idat.extend_from_slice(data);
        } else if kind == b"IEND" {
            break;
        }
        offset = data_end + 4;
    }

    let mut decoder = ZlibDecoder::new(idat.as_slice());
    let mut scanlines = Vec::new();
    decoder.read_to_end(&mut scanlines).unwrap();
    let row_len = width as usize * 4;
    let mut rgba = Vec::with_capacity(row_len * height as usize);
    for row in scanlines.chunks_exact(row_len + 1) {
        assert_eq!(row[0], 0, "renderer PNG uses the None filter");
        rgba.extend_from_slice(&row[1..]);
    }
    (width, height, rgba)
}

fn base64_decode(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    assert_eq!(bytes.len() % 4, 0);
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks_exact(4) {
        let a = base64_value(chunk[0]).unwrap();
        let b = base64_value(chunk[1]).unwrap();
        let c = base64_value(chunk[2]);
        let d = base64_value(chunk[3]);
        output.push((a << 2) | (b >> 4));
        if let Some(c) = c {
            output.push((b << 4) | (c >> 2));
            if let Some(d) = d {
                output.push((c << 6) | d);
            }
        }
    }
    output
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        b'=' => None,
        _ => panic!("invalid base64 byte"),
    }
}
