use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use next_domain::{Document, Element, ElementId, ElementKind, Layer, LayerId, PageId, Rect};
use render_plan::{LayerScope, PlannedElement, RenderPlan, RenderPrimitiveFamily};

use super::{
    MetafileRenditions, SvgDiagnostic, SvgRenderError, SvgRenderOptions, SvgRenderOutput,
    render_plan_to_svg_with_context,
};

pub(super) fn apply_layer_references(
    document: &Document,
    page_id: PageId,
    plan: &RenderPlan<'_>,
    renditions: &MetafileRenditions,
    output: &mut SvgRenderOutput,
    recursion_stack: &mut Vec<ElementId>,
) -> Result<(), SvgRenderError> {
    let Some(current_page_index) = document.pages.iter().position(|page| page.id == page_id) else {
        return Ok(());
    };
    let mut rendered = Vec::new();
    let mut nested_diagnostics = Vec::new();

    // Walk backwards so a later layer-reference fragment is already materialized
    // when an earlier one needs it as its z-order insertion anchor.
    for index in (0..plan.items.len()).rev() {
        let item = &plan.items[index];
        let ElementKind::LayerReference {
            relative_page_index,
            layer_index,
        } = &item.element.kind
        else {
            continue;
        };

        if recursion_stack.contains(&item.element.id)
            || !element_geometry_is_finite(item.element)
        {
            // The pinned upstream object uses a per-instance `Drawing` guard.
            // Leaving the core LayerReference unsupported diagnostic in place is
            // intentional for recursive or malformed invocations: no fallback
            // approximation is emitted.
            continue;
        }

        let Some(target) = resolve_target(
            document,
            current_page_index,
            *relative_page_index,
            *layer_index,
        ) else {
            // Invalid page/layer indices remain explicit typed unsupported
            // diagnostics from the selected core renderer.
            continue;
        };

        let target_plan = plan_layer(target.layer);
        recursion_stack.push(item.element.id);
        let nested_result = render_plan_to_svg_with_context(
            document,
            target.page_id,
            &target_plan,
            SvgRenderOptions::default(),
            renditions,
            recursion_stack,
        );
        recursion_stack.pop();
        let nested = nested_result?;

        append_unique_diagnostics(&mut nested_diagnostics, nested.diagnostics);
        let fragment = render_reference_fragment(
            item.element,
            target.page_index,
            target.layer_index,
            target.page_width_mm,
            target.page_height_mm,
            &nested.svg,
        );
        if inject_fragment_in_plan_order(&mut output.svg, plan, index, &fragment) {
            rendered.push(item.element.id);
        }
    }

    if rendered.is_empty() {
        append_unique_diagnostics(&mut output.diagnostics, nested_diagnostics);
        return Ok(());
    }

    // Remove only the selected core's top-level unsupported diagnostic for
    // references that were actually materialized. Nested diagnostics are appended
    // afterwards so a recursive re-entry of the same element ID stays explicit.
    output.diagnostics.retain(|diagnostic| {
        !matches!(
            diagnostic,
            SvgDiagnostic::UnsupportedPrimitive { element_id, .. }
                if rendered.contains(element_id)
        )
    });
    append_unique_diagnostics(&mut output.diagnostics, nested_diagnostics);
    output.rendered_elements += rendered.len();
    output.skipped_elements = output.skipped_elements.saturating_sub(rendered.len());
    Ok(())
}

struct ResolvedTarget<'a> {
    page_id: PageId,
    page_index: usize,
    layer_index: usize,
    page_width_mm: f64,
    page_height_mm: f64,
    layer: &'a Layer,
}

fn resolve_target(
    document: &Document,
    current_page_index: usize,
    relative_page_index: i32,
    layer_index: i32,
) -> Option<ResolvedTarget<'_>> {
    if layer_index < 0 {
        return None;
    }
    let target_page_index = i64::try_from(current_page_index)
        .ok()?
        .checked_add(i64::from(relative_page_index))?;
    if target_page_index < 0 {
        return None;
    }
    let target_page_index = usize::try_from(target_page_index).ok()?;
    let target_page = document.pages.get(target_page_index)?;
    if !target_page.size_mm.width.is_finite()
        || !target_page.size_mm.height.is_finite()
        || target_page.size_mm.width <= 0.0
        || target_page.size_mm.height <= 0.0
    {
        return None;
    }
    let layer_index = usize::try_from(layer_index).ok()?;
    let layer = target_page.layers.get(layer_index)?;
    Some(ResolvedTarget {
        page_id: target_page.id,
        page_index: target_page_index,
        layer_index,
        page_width_mm: target_page.size_mm.width,
        page_height_mm: target_page.size_mm.height,
        layer,
    })
}

fn plan_layer(layer: &Layer) -> RenderPlan<'_> {
    let elements: BTreeMap<_, _> = layer
        .scene
        .elements
        .iter()
        .map(|element| (element.id, element))
        .collect();
    let mut items = Vec::new();
    let mut visited = BTreeSet::new();
    let mut recursion_stack = BTreeSet::new();
    let mut visited_elements = 0usize;

    for root in &layer.scene.roots {
        visit_layer_element(
            *root,
            layer.id,
            &elements,
            &mut visited,
            &mut recursion_stack,
            &mut items,
            &mut visited_elements,
        );
    }

    RenderPlan {
        items,
        diagnostics: Vec::new(),
        visited_elements,
        culled_elements: 0,
    }
}

fn visit_layer_element<'a>(
    element_id: ElementId,
    layer_id: LayerId,
    elements: &BTreeMap<ElementId, &'a Element>,
    visited: &mut BTreeSet<ElementId>,
    recursion_stack: &mut BTreeSet<ElementId>,
    items: &mut Vec<PlannedElement<'a>>,
    visited_elements: &mut usize,
) {
    if recursion_stack.contains(&element_id) || !visited.insert(element_id) {
        return;
    }
    let Some(element) = elements.get(&element_id).copied() else {
        return;
    };
    *visited_elements += 1;

    if let ElementKind::Group { children } = &element.kind {
        recursion_stack.insert(element_id);
        for child in children {
            visit_layer_element(
                *child,
                layer_id,
                elements,
                visited,
                recursion_stack,
                items,
                visited_elements,
            );
        }
        recursion_stack.remove(&element_id);
        return;
    }

    items.push(PlannedElement {
        layer_id,
        layer_scope: LayerScope::Page,
        family: primitive_family(&element.kind),
        element,
    });
}

fn primitive_family(kind: &ElementKind) -> RenderPrimitiveFamily {
    match kind {
        ElementKind::Text => RenderPrimitiveFamily::Text,
        ElementKind::Rectangle { .. } => RenderPrimitiveFamily::Rectangle,
        ElementKind::Ellipse => RenderPrimitiveFamily::Ellipse,
        ElementKind::StraightConnector { .. } | ElementKind::OrthogonalConnector { .. } => {
            RenderPrimitiveFamily::Connector
        }
        ElementKind::Image { .. } => RenderPrimitiveFamily::Image,
        ElementKind::Metafile { .. } => RenderPrimitiveFamily::Metafile,
        ElementKind::Group { .. } => unreachable!("groups are expanded before primitive classification"),
        ElementKind::Polygon { .. } => RenderPrimitiveFamily::Polygon,
        ElementKind::Flowchart { .. } => RenderPrimitiveFamily::Flowchart,
        ElementKind::Curve { .. } => RenderPrimitiveFamily::Curve,
        ElementKind::LayerReference { .. } => RenderPrimitiveFamily::LayerReference,
    }
}

fn render_reference_fragment(
    element: &Element,
    target_page_index: usize,
    target_layer_index: usize,
    page_width_mm: f64,
    page_height_mm: f64,
    nested_svg: &str,
) -> String {
    let bounds = normalize_rect(element.bounds_mm);
    let inner = svg_inner(nested_svg).unwrap_or_default();
    let mut fragment = format!(
        "<g data-element-id=\"{}\" data-ddn-layer-reference-page=\"{}\" data-ddn-layer-reference-layer=\"{}\"",
        element.id.0, target_page_index, target_layer_index,
    );
    write_rotation(&mut fragment, element, bounds);
    fragment.push('>');
    write!(
        fragment,
        "<svg x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\" overflow=\"visible\">{}\</svg></g>",
        num(bounds.x),
        num(bounds.y),
        num(bounds.width),
        num(bounds.height),
        num(page_width_mm),
        num(page_height_mm),
        inner,
    )
    .expect("writing inherited-layer SVG fragment into String cannot fail");
    fragment
}

fn svg_inner(svg: &str) -> Option<&str> {
    let start = svg.find('>')? + 1;
    let end = svg.rfind("</svg>")?;
    (start <= end).then_some(&svg[start..end])
}

fn append_unique_diagnostics(target: &mut Vec<SvgDiagnostic>, source: Vec<SvgDiagnostic>) {
    for diagnostic in source {
        if !target.contains(&diagnostic) {
            target.push(diagnostic);
        }
    }
}

fn element_geometry_is_finite(element: &Element) -> bool {
    element.bounds_mm.x.is_finite()
        && element.bounds_mm.y.is_finite()
        && element.bounds_mm.width.is_finite()
        && element.bounds_mm.height.is_finite()
        && element.rotation_deg.is_finite()
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

fn write_rotation(target: &mut String, element: &Element, bounds: Rect) {
    if element.rotation_deg == 0.0 {
        return;
    }
    write!(
        target,
        " transform=\"rotate({} {} {})\"",
        num(element.rotation_deg),
        num(bounds.x + bounds.width / 2.0),
        num(bounds.y + bounds.height / 2.0),
    )
    .expect("writing inherited-layer rotation into String cannot fail");
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

fn num(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    let mut value = format!("{value:.4}");
    while value.contains('.') && value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    value
}
