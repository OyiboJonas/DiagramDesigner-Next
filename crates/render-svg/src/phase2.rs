//! Phase-2 production SVG facade.
//!
//! The selected SVG renderer remains the stable production backend. Compatibility
//! semantics are layered outside `next-domain` so renderer-specific state never
//! leaks into editor history or semantic commands.

mod curve;
#[path = "public.rs"]
mod existing;
mod flowchart;
mod layer_reference;
mod metafile;

pub use existing::{
    RasterAssetIssue, SvgDiagnostic, SvgRenderError, SvgRenderOptions, SvgRenderOutput,
};
pub use metafile::{MetafileRendition, MetafileRenditions};

use next_domain::{Document, ElementId, PageId};
use render_plan::RenderPlan;

/// Render with the normal production SVG pipeline.
///
/// Legacy metafiles remain explicit unsupported primitives unless a platform layer
/// supplies a verified browser-renderable rendition via
/// [`render_plan_to_svg_with_metafile_renditions`]. Known public legacy curve,
/// flowchart and inherited-layer families are materialized by renderer-local
/// Phase-2 compatibility layers.
pub fn render_plan_to_svg(
    document: &Document,
    page_id: PageId,
    plan: &RenderPlan<'_>,
    options: SvgRenderOptions,
) -> Result<SvgRenderOutput, SvgRenderError> {
    render_plan_to_svg_with_metafile_renditions(
        document,
        page_id,
        plan,
        options,
        &MetafileRenditions::new(),
    )
}

/// Render with optional platform-produced metafile renditions.
///
/// Renditions are keyed by stable `AssetId` and are consumed only by the renderer.
/// The original binary asset remains the persisted source of truth. Supplying a
/// rendition therefore does not, by itself, claim general WMF/EMF compatibility.
pub fn render_plan_to_svg_with_metafile_renditions(
    document: &Document,
    page_id: PageId,
    plan: &RenderPlan<'_>,
    options: SvgRenderOptions,
    renditions: &MetafileRenditions,
) -> Result<SvgRenderOutput, SvgRenderError> {
    render_plan_to_svg_with_context(
        document,
        page_id,
        plan,
        options,
        renditions,
        &mut Vec::new(),
    )
}

fn render_plan_to_svg_with_context(
    document: &Document,
    page_id: PageId,
    plan: &RenderPlan<'_>,
    options: SvgRenderOptions,
    renditions: &MetafileRenditions,
    layer_reference_stack: &mut Vec<ElementId>,
) -> Result<SvgRenderOutput, SvgRenderError> {
    let mut output = existing::render_plan_to_svg(document, page_id, plan, options)?;
    curve::apply_curves(document, plan, &mut output);
    flowchart::apply_flowcharts(document, plan, &mut output);
    metafile::apply_metafiles(document, plan, renditions, &mut output);
    layer_reference::apply_layer_references(
        document,
        page_id,
        plan,
        renditions,
        &mut output,
        layer_reference_stack,
    )?;
    Ok(output)
}
