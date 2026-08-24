use std::fmt::Write as _;

use next_domain::{Document, Element, ElementId, ElementKind, ElementStyle, GradientAxis, Rect};
use render_plan::RenderPlan;

use super::{
    DEFAULT_STROKE_MM, Paint, SvgDiagnostic, SvgRenderOutput, inject_defs, num, paint_attributes,
    resolve_paint, rotation_attribute,
};

pub(super) fn apply_polygons(
    document: &Document,
    plan: &RenderPlan<'_>,
    output: &mut SvgRenderOutput,
) {
    let mut supported = Vec::new();
    let mut malformed = Vec::new();
    let mut defs = String::new();

    // The Phase-1 core skips polygons. Work backwards so insertion before the
    // nearest later materialized element preserves the render-plan z-order.
    for index in (0..plan.items.len()).rev() {
        let item = &plan.items[index];
        let ElementKind::Polygon { vertices } = &item.element.kind else {
            continue;
        };

        if !polygon_geometry_is_valid(item.element, vertices) {
            malformed.push(item.element.id);
            push_invalid_geometry_once(output, item.element.id);
            continue;
        }

        let style = item
            .element
            .style_id
            .and_then(|style_id| document.styles.iter().find(|style| style.id == style_id));
        let fragment = render_polygon(
            item.element,
            vertices,
            style,
            &mut defs,
            &mut output.diagnostics,
        );
        if inject_fragment_in_plan_order(&mut output.svg, plan, index, &fragment) {
            supported.push(item.element.id);
        }
    }

    if !defs.is_empty() {
        inject_defs(&mut output.svg, &defs);
    }

    if supported.is_empty() && malformed.is_empty() {
        return;
    }

    output.diagnostics.retain(|diagnostic| {
        !matches!(
            diagnostic,
            SvgDiagnostic::UnsupportedPrimitive { element_id, .. }
                if supported.contains(element_id) || malformed.contains(element_id)
        )
    });
    output.rendered_elements += supported.len();
    output.skipped_elements = output.skipped_elements.saturating_sub(supported.len());
}

fn polygon_geometry_is_valid(element: &Element, vertices: &[next_domain::NormalizedPoint]) -> bool {
    element.bounds_mm.x.is_finite()
        && element.bounds_mm.y.is_finite()
        && element.bounds_mm.width.is_finite()
        && element.bounds_mm.height.is_finite()
        && element.rotation_deg.is_finite()
        && vertices.len() >= 2
        && vertices
            .iter()
            .all(|vertex| vertex.x.is_finite() && vertex.y.is_finite())
}

fn push_invalid_geometry_once(output: &mut SvgRenderOutput, element_id: ElementId) {
    if output.diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic,
            SvgDiagnostic::InvalidGeometry {
                element_id: existing_element_id,
            } if *existing_element_id == element_id
        )
    }) {
        return;
    }
    output
        .diagnostics
        .push(SvgDiagnostic::InvalidGeometry { element_id });
}

fn render_polygon(
    element: &Element,
    vertices: &[next_domain::NormalizedPoint],
    style: Option<&ElementStyle>,
    defs: &mut String,
    diagnostics: &mut Vec<SvgDiagnostic>,
) -> String {
    let bounds = normalize_rect(element.bounds_mm);
    let points = vertices
        .iter()
        .map(|vertex| {
            format!(
                "{},{}",
                num(bounds.x + vertex.x * bounds.width),
                num(bounds.y + vertex.y * bounds.height)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");

    let mut result = format!(
        "<polygon data-element-id=\"{}\" points=\"{}\"",
        element.id.0, points
    );
    write_stroke(&mut result, element.id, style, diagnostics);
    write_fill(&mut result, defs, element.id, style, diagnostics);
    result.push_str(&rotation_attribute(element));
    result.push_str("/>");
    result
}

fn write_stroke(
    target: &mut String,
    element_id: ElementId,
    style: Option<&ElementStyle>,
    diagnostics: &mut Vec<SvgDiagnostic>,
) {
    match style {
        Some(style) => match style.stroke.as_ref() {
            Some(stroke) => {
                let paint = resolve_with_diagnostic(&stroke.color, element_id, diagnostics);
                write!(
                    target,
                    " {} stroke-width=\"{}\"",
                    paint_attributes("stroke", &paint),
                    num(finite_non_negative(stroke.width_mm, DEFAULT_STROKE_MM)),
                )
                .expect("writing polygon stroke into String cannot fail");
            }
            None => target.push_str(" stroke=\"none\""),
        },
        None => target.push_str(" stroke=\"#000000\" stroke-width=\"0.25\""),
    }
}

fn write_fill(
    target: &mut String,
    defs: &mut String,
    element_id: ElementId,
    style: Option<&ElementStyle>,
    diagnostics: &mut Vec<SvgDiagnostic>,
) {
    let Some(fill) = style.and_then(|style| style.fill.as_ref()) else {
        target.push_str(" fill=\"none\"");
        return;
    };

    let start = resolve_with_diagnostic(&fill.color, element_id, diagnostics);
    let Some(gradient) = fill.gradient.as_ref() else {
        write!(target, " {}", paint_attributes("fill", &start))
            .expect("writing polygon fill into String cannot fail");
        return;
    };

    let end = resolve_with_diagnostic(&gradient.end_color, element_id, diagnostics);
    let gradient_id = format!("ddn-polygon-gradient-{}", element_id.0);
    let (x1, y1, x2, y2) = match gradient.axis {
        GradientAxis::AlongX => ("0%", "0%", "100%", "0%"),
        GradientAxis::AlongY => ("0%", "0%", "0%", "100%"),
    };
    write!(
        defs,
        "<linearGradient id=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">",
        gradient_id, x1, y1, x2, y2
    )
    .expect("writing polygon gradient into String cannot fail");
    write_gradient_stop(defs, "0%", &start);
    write_gradient_stop(defs, "100%", &end);
    defs.push_str("</linearGradient>");
    write!(target, " fill=\"url(#{gradient_id})\"")
        .expect("writing polygon gradient reference into String cannot fail");
}

fn write_gradient_stop(target: &mut String, offset: &str, paint: &Paint) {
    write!(
        target,
        "<stop offset=\"{}\" stop-color=\"{}\"",
        offset, paint.css
    )
    .expect("writing polygon gradient stop into String cannot fail");
    if paint.opacity < 1.0 {
        write!(target, " stop-opacity=\"{}\"", num(paint.opacity))
            .expect("writing polygon gradient opacity into String cannot fail");
    }
    target.push_str("/>");
}

fn resolve_with_diagnostic(
    color: &next_domain::Color,
    element_id: ElementId,
    diagnostics: &mut Vec<SvgDiagnostic>,
) -> Paint {
    let (paint, palette_index) = resolve_paint(color);
    if let Some(index) = palette_index {
        let already_reported = diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic,
                SvgDiagnostic::SystemPaletteFallback {
                    element_id: existing_element_id,
                    index: existing_index,
                } if *existing_element_id == element_id && *existing_index == index
            )
        });
        if !already_reported {
            diagnostics.push(SvgDiagnostic::SystemPaletteFallback { element_id, index });
        }
    }
    paint
}

fn finite_non_negative(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        fallback
    }
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
