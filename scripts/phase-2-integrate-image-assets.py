from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file_path = Path(path)
    text = file_path.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"expected text not found in {path}: {old[:120]!r}")
    file_path.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "crates/render-svg/src/public.rs",
    "mod core;\nmod orthogonal;\nmod polygon;\n",
    "mod core;\nmod image;\nmod orthogonal;\nmod polygon;\n",
)
replace_once(
    "crates/render-svg/src/public.rs",
    "pub use core::{SvgDiagnostic, SvgRenderError, SvgRenderOptions, SvgRenderOutput};",
    "pub use core::{\n    RasterAssetIssue, SvgDiagnostic, SvgRenderError, SvgRenderOptions, SvgRenderOutput,\n};",
)
replace_once(
    "crates/render-svg/src/public.rs",
    "    polygon::apply_polygons(document, plan, &mut output);\n    orthogonal::apply_orthogonal_connectors(document, plan, &mut output);",
    "    polygon::apply_polygons(document, plan, &mut output);\n    image::apply_images(document, plan, &mut output);\n    orthogonal::apply_orthogonal_connectors(document, plan, &mut output);",
)

replace_once(
    "crates/render-svg/src/lib.rs",
    "use next_domain::{\n    Color, Document, Element, ElementId, ElementKind, ElementStyle, FillStyle, GradientAxis,",
    "use next_domain::{\n    AssetId, Color, Document, Element, ElementId, ElementKind, ElementStyle, FillStyle, GradientAxis,",
)
options_block = """#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SvgRenderOptions {
    /// SVG view box in document millimetres. `None` uses the complete page.
    pub view_box_mm: Option<Rect>,
}
"""
issue_block = options_block + """
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterAssetIssue {
    InvalidDimensions,
    SizeOverflow,
    UnsupportedBitsPerPixel { bits_per_pixel: u8 },
    MissingPalette,
    InvalidPaletteLength { expected: usize, actual: usize },
    InvalidPixelLength { expected: usize, actual: usize },
    InvalidAlphaLength { expected: usize, actual: usize },
    EncodingFailed,
}
"""
replace_once("crates/render-svg/src/lib.rs", options_block, issue_block)
replace_once(
    "crates/render-svg/src/lib.rs",
    """    InvalidGeometry {
        element_id: ElementId,
    },
""",
    """    InvalidGeometry {
        element_id: ElementId,
    },
    MissingAsset {
        element_id: ElementId,
        asset_id: AssetId,
    },
    UnsupportedAssetPayload {
        element_id: ElementId,
        asset_id: AssetId,
    },
    InvalidRasterAsset {
        element_id: ElementId,
        asset_id: AssetId,
        issue: RasterAssetIssue,
    },
""",
)

replace_once(
    "crates/render-svg/Cargo.toml",
    "[dependencies]\nnext-domain = { path = \"../next-domain\" }",
    "[dependencies]\nflate2.workspace = true\nnext-domain = { path = \"../next-domain\" }",
)
replace_once(
    "Cargo.lock",
    """name = "render-svg"
version = "0.1.0"
dependencies = [
 "next-domain",
""",
    """name = "render-svg"
version = "0.1.0"
dependencies = [
 "flate2",
 "next-domain",
""",
)

replace_once(
    "crates/render-svg/src/image.rs",
    """        let ElementKind::Image { asset_id } = item.element.kind else {
            continue;
        };
""",
    """        let ElementKind::Image { asset_id } = &item.element.kind else {
            continue;
        };
        let asset_id = *asset_id;
""",
)

replace_once(
    "crates/render-svg/examples/fidelity_scene.rs",
    "    AnchorSet, Color, Connector, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId,\n",
    "    AnchorSet, Asset, AssetId, AssetPayload, Color, Connector, ConnectorLabelStyle, Document,\n    DocumentDefaults, DocumentId,\n",
)
replace_once(
    "crates/render-svg/examples/fidelity_scene.rs",
    "    let polygon_id = ElementId::v5(namespace, \"adr-019-fidelity/element/polygon\");\n",
    "    let polygon_id = ElementId::v5(namespace, \"adr-019-fidelity/element/polygon\");\n    let raster_image_id = ElementId::v5(namespace, \"adr-019-fidelity/element/raster-image\");\n    let raster_asset_id = AssetId::v5(namespace, \"adr-019-fidelity/asset/raster-image\");\n",
)
replace_once(
    "crates/render-svg/examples/fidelity_scene.rs",
    "                text: \"Arrow marker and polygon are rendered by the Phase-2 production facade\"\n                    .to_owned(),",
    "                text: \"Arrow marker, polygon and raster image are rendered by the Phase-2 production facade\"\n                    .to_owned(),",
)
raster_element = """
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
"""
replace_once(
    "crates/render-svg/examples/fidelity_scene.rs",
    "\n    // Four partially clipped, rotated rectangles make page-edge/culling mistakes",
    raster_element + "\n    // Four partially clipped, rotated rectangles make page-edge/culling mistakes",
)
replace_once(
    "crates/render-svg/examples/fidelity_scene.rs",
    "        polygon,\n        edge_left,",
    "        polygon,\n        raster_image,\n        edge_left,",
)
raster_asset = """assets: vec![Asset {
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
        }],"""
replace_once(
    "crates/render-svg/examples/fidelity_scene.rs",
    "assets: Vec::new(),",
    raster_asset,
)
