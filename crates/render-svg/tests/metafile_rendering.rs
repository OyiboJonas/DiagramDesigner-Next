use next_domain::{
    AnchorSet, Asset, AssetId, AssetPayload, ConnectorLabelStyle, Document, DocumentDefaults,
    DocumentId, Element, ElementId, ElementKind, Layer, LayerId, Page, Rect, Scene, Size,
};
use render_plan::{RenderPlanOptions, RenderPrimitiveFamily, build_page_plan};
use render_svg::{
    MetafileRendition, MetafileRenditions, SvgDiagnostic, SvgRenderOptions, SvgRenderOutput,
    render_plan_to_svg, render_plan_to_svg_with_metafile_renditions,
};

const METAFILE_MEDIA_TYPE: &str = "application/vnd.diagramdesigner-next.windows-metafile";

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

fn metafile(id: ElementId, asset_id: AssetId, x: f64) -> Element {
    element(id, x, ElementKind::Metafile { asset_id })
}

fn binary_asset(id: AssetId, media_type: &str, bytes: Vec<u8>) -> Asset {
    Asset {
        id,
        sha256: "synthetic-metafile".to_owned(),
        media_type: media_type.to_owned(),
        payload: AssetPayload::Binary { bytes },
    }
}

fn document(elements: Vec<Element>, assets: Vec<Asset>) -> (Document, next_domain::PageId) {
    let page_id = next_domain::PageId::new();
    let roots = elements.iter().map(|element| element.id).collect();
    (
        Document {
            id: DocumentId::new(),
            name: "Metafile regression".to_owned(),
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

fn render(document: &Document, page_id: next_domain::PageId) -> SvgRenderOutput {
    let plan = build_page_plan(document, page_id, RenderPlanOptions::default()).unwrap();
    render_plan_to_svg(document, page_id, &plan, SvgRenderOptions::default()).unwrap()
}

fn render_with_renditions(
    document: &Document,
    page_id: next_domain::PageId,
    renditions: &MetafileRenditions,
) -> SvgRenderOutput {
    let plan = build_page_plan(document, page_id, RenderPlanOptions::default()).unwrap();
    render_plan_to_svg_with_metafile_renditions(
        document,
        page_id,
        &plan,
        SvgRenderOptions::default(),
        renditions,
    )
    .unwrap()
}

#[test]
fn preserved_metafile_without_rendition_stays_explicitly_unsupported() {
    let asset_id = AssetId::new();
    let metafile_id = ElementId::new();
    let asset = binary_asset(asset_id, METAFILE_MEDIA_TYPE, vec![1, 2, 3, 4]);
    let (document, page_id) = document(vec![metafile(metafile_id, asset_id, 10.0)], vec![asset]);
    let output = render(&document, page_id);

    assert_eq!(output.rendered_elements, 0);
    assert_eq!(output.skipped_elements, 1);
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive {
            element_id,
            family: RenderPrimitiveFamily::Metafile,
        } if *element_id == metafile_id
    )));
    assert!(!output.svg.contains("data-ddn-metafile-rendition="));
}

#[test]
fn supplied_web_rendition_uses_legacy_stretch_and_center_rotation_contract() {
    let asset_id = AssetId::new();
    let metafile_id = ElementId::new();
    let asset = binary_asset(asset_id, METAFILE_MEDIA_TYPE, vec![0xd7, 0xcd, 0xc6, 0x9a]);
    let mut element = metafile(metafile_id, asset_id, 10.0);
    element.rotation_deg = 90.0;
    let (document, page_id) = document(vec![element], vec![asset]);
    let renditions = MetafileRenditions::from([(
        asset_id,
        MetafileRendition {
            media_type: "image/png".to_owned(),
            bytes: vec![1, 2, 3],
        },
    )]);
    let output = render_with_renditions(&document, page_id, &renditions);

    assert_eq!(output.rendered_elements, 1);
    assert_eq!(output.skipped_elements, 0);
    assert!(output.svg.contains("<image"));
    assert!(
        output
            .svg
            .contains(&format!("data-ddn-asset-id=\"{}\"", asset_id.0))
    );
    assert!(
        output
            .svg
            .contains("data-ddn-metafile-rendition=\"image/png\"")
    );
    assert!(
        output
            .svg
            .contains("x=\"10\" y=\"10\" width=\"20\" height=\"12\"")
    );
    assert!(output.svg.contains("preserveAspectRatio=\"none\""));
    assert!(output.svg.contains("transform=\"rotate(90 20 16)\""));
    assert!(output.svg.contains("href=\"data:image/png;base64,AQID\""));
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive { element_id, .. } if *element_id == metafile_id
    )));
}

#[test]
fn missing_or_invalid_source_assets_get_asset_diagnostics_not_false_compatibility() {
    let missing_asset_id = AssetId::new();
    let wrong_media_asset_id = AssetId::new();
    let empty_asset_id = AssetId::new();
    let missing_id = ElementId::new();
    let wrong_media_id = ElementId::new();
    let empty_id = ElementId::new();

    let wrong_media = binary_asset(wrong_media_asset_id, "application/octet-stream", vec![1]);
    let empty = binary_asset(empty_asset_id, METAFILE_MEDIA_TYPE, Vec::new());
    let (document, page_id) = document(
        vec![
            metafile(missing_id, missing_asset_id, 10.0),
            metafile(wrong_media_id, wrong_media_asset_id, 40.0),
            metafile(empty_id, empty_asset_id, 70.0),
        ],
        vec![wrong_media, empty],
    );
    let output = render(&document, page_id);

    assert_eq!(output.rendered_elements, 0);
    assert_eq!(output.skipped_elements, 3);
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::MissingAsset { element_id, asset_id }
            if *element_id == missing_id && *asset_id == missing_asset_id
    )));
    for (element_id, asset_id) in [
        (wrong_media_id, wrong_media_asset_id),
        (empty_id, empty_asset_id),
    ] {
        assert!(output.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            SvgDiagnostic::UnsupportedAssetPayload {
                element_id: diagnostic_element_id,
                asset_id: diagnostic_asset_id,
            } if *diagnostic_element_id == element_id && *diagnostic_asset_id == asset_id
        )));
    }
    assert!(!output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive { element_id, .. }
            if [missing_id, wrong_media_id, empty_id].contains(element_id)
    )));
}

#[test]
fn invalid_rendition_is_not_treated_as_metafile_support() {
    let asset_id = AssetId::new();
    let metafile_id = ElementId::new();
    let asset = binary_asset(asset_id, METAFILE_MEDIA_TYPE, vec![1, 2, 3]);
    let (document, page_id) = document(vec![metafile(metafile_id, asset_id, 10.0)], vec![asset]);
    let renditions = MetafileRenditions::from([(
        asset_id,
        MetafileRendition {
            media_type: "image/wmf".to_owned(),
            bytes: vec![1, 2, 3],
        },
    )]);
    let output = render_with_renditions(&document, page_id, &renditions);

    assert_eq!(output.rendered_elements, 0);
    assert_eq!(output.skipped_elements, 1);
    assert!(output.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SvgDiagnostic::UnsupportedPrimitive {
            element_id,
            family: RenderPrimitiveFamily::Metafile,
        } if *element_id == metafile_id
    )));
}

#[test]
fn rendition_preserves_render_plan_z_order() {
    let asset_id = AssetId::new();
    let before_id = ElementId::new();
    let metafile_id = ElementId::new();
    let after_id = ElementId::new();
    let asset = binary_asset(asset_id, METAFILE_MEDIA_TYPE, vec![1]);
    let before = element(
        before_id,
        5.0,
        ElementKind::Rectangle {
            corner_radius_mm: 0.0,
        },
    );
    let after = element(
        after_id,
        70.0,
        ElementKind::Rectangle {
            corner_radius_mm: 0.0,
        },
    );
    let (document, page_id) = document(
        vec![before, metafile(metafile_id, asset_id, 40.0), after],
        vec![asset],
    );
    let renditions = MetafileRenditions::from([(
        asset_id,
        MetafileRendition {
            media_type: "image/png".to_owned(),
            bytes: vec![1],
        },
    )]);
    let output = render_with_renditions(&document, page_id, &renditions);

    let before_pos = output.svg.find(&before_id.0.to_string()).unwrap();
    let metafile_pos = output.svg.find(&metafile_id.0.to_string()).unwrap();
    let after_pos = output.svg.find(&after_id.0.to_string()).unwrap();
    assert!(before_pos < metafile_pos && metafile_pos < after_pos);
}
