use std::collections::BTreeMap;

use next_domain::{Asset, AssetId, AssetPayload, Document, Element, ElementKind, Rect};
use render_plan::RenderPlan;

use super::{
    MetafileAssetIssue, MetafileRenditionIssue, SvgDiagnostic, SvgRenderOutput, num,
    rotation_attribute,
};

const LEGACY_METAFILE_MEDIA_TYPE: &str =
    "application/vnd.diagramdesigner-next.windows-metafile";
const SUPPORTED_RENDITION_MEDIA_TYPES: [&str; 4] = [
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/svg+xml",
];

/// Renderer-local, disposable web rendition of one preserved legacy metafile asset.
///
/// The rendition is deliberately not part of `next-domain`: the original WMF/EMF
/// bytes remain the persisted source of truth while platform code may produce a
/// browser-renderable representation for the current session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetafileRendition {
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// Renditions keyed by the stable source asset identity.
pub type MetafileRenditions = BTreeMap<AssetId, MetafileRendition>;

pub(super) fn apply_metafiles(
    document: &Document,
    plan: &RenderPlan<'_>,
    renditions: &MetafileRenditions,
    output: &mut SvgRenderOutput,
) {
    let mut retired = Vec::new();
    let mut rendered = Vec::new();

    // The Phase-1 core skips metafiles. Work backwards so inserting before the
    // nearest later materialized element preserves render-plan z-order.
    for index in (0..plan.items.len()).rev() {
        let item = &plan.items[index];
        let ElementKind::Metafile { asset_id } = &item.element.kind else {
            continue;
        };
        let asset_id = *asset_id;

        // The core already owns invalid element-geometry diagnostics and does not
        // emit UnsupportedPrimitive for those elements, so leave them untouched.
        if !element_geometry_is_finite(item.element) {
            continue;
        }

        let Some(asset) = document.assets.iter().find(|asset| asset.id == asset_id) else {
            push_diagnostic_once(
                output,
                SvgDiagnostic::MissingAsset {
                    element_id: item.element.id,
                    asset_id,
                },
            );
            retired.push(item.element.id);
            continue;
        };

        if let Err(issue) = validate_source_asset(asset) {
            push_diagnostic_once(
                output,
                SvgDiagnostic::InvalidMetafileAsset {
                    element_id: item.element.id,
                    asset_id,
                    issue,
                },
            );
            retired.push(item.element.id);
            continue;
        }

        let Some(rendition) = renditions.get(&asset_id) else {
            push_diagnostic_once(
                output,
                SvgDiagnostic::MetafileRenditionUnavailable {
                    element_id: item.element.id,
                    asset_id,
                },
            );
            retired.push(item.element.id);
            continue;
        };

        if let Err(issue) = validate_rendition(rendition) {
            push_diagnostic_once(
                output,
                SvgDiagnostic::InvalidMetafileRendition {
                    element_id: item.element.id,
                    asset_id,
                    issue,
                },
            );
            retired.push(item.element.id);
            continue;
        }

        let fragment = render_metafile_rendition(item.element, asset_id, rendition);
        if inject_fragment_in_plan_order(&mut output.svg, plan, index, &fragment) {
            retired.push(item.element.id);
            rendered.push(item.element.id);
        }
    }

    if retired.is_empty() {
        return;
    }

    output.diagnostics.retain(|diagnostic| {
        !matches!(
            diagnostic,
            SvgDiagnostic::UnsupportedPrimitive { element_id, .. }
                if retired.contains(element_id)
        )
    });
    output.rendered_elements += rendered.len();
    output.skipped_elements = output.skipped_elements.saturating_sub(rendered.len());
}

fn validate_source_asset(asset: &Asset) -> Result<(), MetafileAssetIssue> {
    if asset.media_type != LEGACY_METAFILE_MEDIA_TYPE {
        return Err(MetafileAssetIssue::UnexpectedMediaType {
            actual: asset.media_type.clone(),
        });
    }

    match &asset.payload {
        AssetPayload::Binary { bytes } if bytes.is_empty() => Err(MetafileAssetIssue::EmptyPayload),
        AssetPayload::Binary { .. } => Ok(()),
        AssetPayload::Raster { .. } => Err(MetafileAssetIssue::ExpectedBinaryPayload),
    }
}

fn validate_rendition(rendition: &MetafileRendition) -> Result<(), MetafileRenditionIssue> {
    if rendition.bytes.is_empty() {
        return Err(MetafileRenditionIssue::EmptyPayload);
    }
    if !SUPPORTED_RENDITION_MEDIA_TYPES.contains(&rendition.media_type.as_str()) {
        return Err(MetafileRenditionIssue::UnsupportedMediaType {
            actual: rendition.media_type.clone(),
        });
    }
    Ok(())
}

fn render_metafile_rendition(
    element: &Element,
    asset_id: AssetId,
    rendition: &MetafileRendition,
) -> String {
    let bounds = normalize_rect(element.bounds_mm);
    let encoded = base64_encode(&rendition.bytes);
    let mut fragment = format!(
        "<image data-element-id=\"{}\" data-ddn-asset-id=\"{}\" data-ddn-metafile-rendition=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"data:{};base64,{}\"",
        element.id.0,
        asset_id.0,
        rendition.media_type,
        num(bounds.x),
        num(bounds.y),
        num(bounds.width),
        num(bounds.height),
        rendition.media_type,
        encoded,
    );
    fragment.push_str(&rotation_attribute(element));
    fragment.push_str("/>");
    fragment
}

fn push_diagnostic_once(output: &mut SvgRenderOutput, diagnostic: SvgDiagnostic) {
    if !output.diagnostics.contains(&diagnostic) {
        output.diagnostics.push(diagnostic);
    }
}

fn element_geometry_is_finite(element: &Element) -> bool {
    element.bounds_mm.x.is_finite()
        && element.bounds_mm.y.is_finite()
        && element.bounds_mm.width.is_finite()
        && element.bounds_mm.height.is_finite()
        && element.rotation_deg.is_finite()
}

fn inject_fragment_in_plan_order(
    svg: &mut String,
    plan: &RenderPlan<'_>,
    item_index: usize,
    fragment: &str,
) -> bool {
    for later in &plan.items[item_index + 1..] {
        let needle = format!("data-element-id=\"{}\"", later.element.id.0);
        let Some(attribute_at) = svg.find(&needle) else {
            continue;
        };
        let Some(tag_start) = svg[..attribute_at].rfind('<') else {
            continue;
        };
        svg.insert_str(tag_start, fragment);
        return true;
    }

    if let Some(end_svg) = svg.rfind("</svg>") {
        svg.insert_str(end_svg, fragment);
        return true;
    }
    false
}

fn normalize_rect(rect: Rect) -> Rect {
    let (x, width) = if rect.width >= 0.0 {
        (rect.x, rect.width)
    } else {
        (rect.x + rect.width, -rect.width)
    };
    let (y, height) = if rect.height >= 0.0 {
        (rect.y, rect.height)
    } else {
        (rect.y + rect.height, -rect.height)
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut index = 0usize;
    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = bytes.get(index + 1).copied();
        let b2 = bytes.get(index + 2).copied();
        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(TABLE[(((b0 & 0x03) << 4) | (b1.unwrap_or(0) >> 4)) as usize] as char);
        if let Some(b1) = b1 {
            output.push(TABLE[(((b1 & 0x0f) << 2) | (b2.unwrap_or(0) >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if let Some(b2) = b2 {
            output.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
        index += 3;
    }
    output
}
