use std::{f64::consts::PI, fmt::Write as _};

use next_domain::{Color, Document, Element, ElementId, ElementKind, ElementStyle, GradientAxis, Rect};
use render_plan::RenderPlan;

use super::{SvgDiagnostic, SvgRenderOutput};

const DEFAULT_STROKE_MM: f64 = 0.25;
const SYSTEM_PALETTE_FALLBACK: &str = "#808080";
const SHAPE_KEY_PREFIX: &str = "builtin:diagramdesigner-flowchart/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowchartKind {
    SideBars,
    Rounded1,
    Rounded2,
    Rounded3,
    SlantRight,
    SlantLeft,
    OddRounded1,
    OddRounded2,
}

impl FlowchartKind {
    fn key(self) -> &'static str {
        match self {
            Self::SideBars => "side-bars",
            Self::Rounded1 => "rounded-1",
            Self::Rounded2 => "rounded-2",
            Self::Rounded3 => "rounded-3",
            Self::SlantRight => "slant-right",
            Self::SlantLeft => "slant-left",
            Self::OddRounded1 => "odd-rounded-1",
            Self::OddRounded2 => "odd-rounded-2",
        }
    }
}

pub(super) fn apply_flowcharts(
    document: &Document,
    plan: &RenderPlan<'_>,
    output: &mut SvgRenderOutput,
) {
    let mut supported = Vec::new();
    let mut defs = String::new();

    // The selected core skips flowcharts. Walk backwards and insert before the
    // nearest later materialized element so the render-plan z-order is unchanged.
    for index in (0..plan.items.len()).rev() {
        let item = &plan.items[index];
        let ElementKind::Flowchart { shape_key } = &item.element.kind else {
            continue;
        };
        let Some(kind) = parse_flowchart_kind(shape_key) else {
            // Unknown/custom flowchart codes remain explicit unsupported
            // primitives. No generic rectangle approximation is emitted.
            continue;
        };
        if !element_geometry_is_finite(item.element) {
            continue;
        }

        let style = item
            .element
            .style_id
            .and_then(|style_id| document.styles.iter().find(|style| style.id == style_id));
        let fragment = render_flowchart(
            item.element,
            kind,
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
    if supported.is_empty() {
        return;
    }

    output.diagnostics.retain(|diagnostic| {
        !matches!(
            diagnostic,
            SvgDiagnostic::UnsupportedPrimitive { element_id, .. }
                if supported.contains(element_id)
        )
    });
    output.rendered_elements += supported.len();
    output.skipped_elements = output.skipped_elements.saturating_sub(supported.len());
}

fn parse_flowchart_kind(shape_key: &str) -> Option<FlowchartKind> {
    let code = shape_key.strip_prefix(SHAPE_KEY_PREFIX)?.parse::<i32>().ok()?;
    match code {
        0x11 => Some(FlowchartKind::SideBars),
        0x21 => Some(FlowchartKind::Rounded1),
        0x22 => Some(FlowchartKind::Rounded2),
        0x23 => Some(FlowchartKind::Rounded3),
        0x31 => Some(FlowchartKind::SlantRight),
        0x32 => Some(FlowchartKind::SlantLeft),
        0x41 => Some(FlowchartKind::OddRounded1),
        0x51 => Some(FlowchartKind::OddRounded2),
        _ => None,
    }
}

fn render_flowchart(
    element: &Element,
    kind: FlowchartKind,
    style: Option<&ElementStyle>,
    defs: &mut String,
    diagnostics: &mut Vec<SvgDiagnostic>,
) -> String {
    let bounds = normalize_rect(element.bounds_mm);
    let rotation = rotation_attribute(element, bounds);
    let mut result = format!(
        "<g data-element-id=\"{}\" data-ddn-flowchart-kind=\"{}\"{}>",
        element.id.0,
        kind.key(),
        rotation,
    );

    match kind {
        FlowchartKind::SideBars => {
            write!(
                result,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                num(bounds.x),
                num(bounds.y),
                num(bounds.width),
                num(bounds.height),
            )
            .expect("writing flowchart rectangle into String cannot fail");
            write_shape_paint(&mut result, defs, element.id, style, diagnostics);
            result.push_str("/>");

            if let Some(stroke) = stroke_attributes(element.id, style, diagnostics) {
                let offset = bounds.width / 8.0;
                for x in [bounds.x + offset, bounds.x + bounds.width - offset] {
                    write!(
                        result,
                        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" fill=\"none\" {} />",
                        num(x),
                        num(bounds.y),
                        num(x),
                        num(bounds.y + bounds.height),
                        stroke,
                    )
                    .expect("writing flowchart side bar into String cannot fail");
                }
            }
        }
        FlowchartKind::Rounded1 | FlowchartKind::Rounded2 | FlowchartKind::Rounded3 => {
            let radius = match kind {
                FlowchartKind::Rounded1 => bounds.height / 2.0,
                FlowchartKind::Rounded2 => bounds.height / 4.0,
                FlowchartKind::Rounded3 => bounds.height / 8.0,
                _ => unreachable!(),
            }
            .max(0.0)
            .min(bounds.width / 2.0)
            .min(bounds.height / 2.0);
            write!(
                result,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\"",
                num(bounds.x),
                num(bounds.y),
                num(bounds.width),
                num(bounds.height),
                num(radius),
                num(radius),
            )
            .expect("writing rounded flowchart into String cannot fail");
            write_shape_paint(&mut result, defs, element.id, style, diagnostics);
            result.push_str("/>");
        }
        FlowchartKind::SlantRight | FlowchartKind::SlantLeft => {
            let offset = bounds.height / 8.0;
            let points = match kind {
                FlowchartKind::SlantRight => vec![
                    (bounds.x + offset, bounds.y),
                    (bounds.x + bounds.width + offset, bounds.y),
                    (bounds.x + bounds.width - offset, bounds.y + bounds.height),
                    (bounds.x - offset, bounds.y + bounds.height),
                ],
                FlowchartKind::SlantLeft => vec![
                    (bounds.x - offset, bounds.y),
                    (bounds.x + bounds.width - offset, bounds.y),
                    (bounds.x + bounds.width + offset, bounds.y + bounds.height),
                    (bounds.x + offset, bounds.y + bounds.height),
                ],
                _ => unreachable!(),
            };
            write_polygon(
                &mut result,
                defs,
                element.id,
                &points,
                style,
                diagnostics,
            );
        }
        FlowchartKind::OddRounded1 | FlowchartKind::OddRounded2 => {
            let points = odd_rounded_points(bounds, kind);
            write_polygon(
                &mut result,
                defs,
                element.id,
                &points,
                style,
                diagnostics,
            );
        }
    }

    result.push_str("</g>");
    result
}

fn odd_rounded_points(bounds: Rect, kind: FlowchartKind) -> Vec<(f64, f64)> {
    let radius = bounds.height / 2.0;
    let mut points = vec![(0.0, 0.0); 32];
    for index in 0..8 {
        let angle = index as f64 * PI / 15.0;
        let x = radius * (1.0 - angle.sin());
        let y = radius * (1.0 - angle.cos());
        points[index] = (bounds.x + x, bounds.y + y);
        points[15 - index] = (bounds.x + x, bounds.y + bounds.height - y);

        let right_x = match kind {
            FlowchartKind::OddRounded1 => bounds.x + bounds.width - x / 2.0,
            FlowchartKind::OddRounded2 => bounds.x + bounds.width + x,
            _ => unreachable!(),
        };
        points[16 + index] = (right_x, bounds.y + bounds.height - y);
        points[31 - index] = (right_x, bounds.y + y);
    }
    points
}

fn write_polygon(
    target: &mut String,
    defs: &mut String,
    element_id: ElementId,
    points: &[(f64, f64)],
    style: Option<&ElementStyle>,
    diagnostics: &mut Vec<SvgDiagnostic>,
) {
    let points = points
        .iter()
        .map(|(x, y)| format!("{},{}", num(*x), num(*y)))
        .collect::<Vec<_>>()
        .join(" ");
    write!(target, "<polygon points=\"{}\"", points)
        .expect("writing flowchart polygon into String cannot fail");
    write_shape_paint(target, defs, element_id, style, diagnostics);
    target.push_str("/>");
}

fn write_shape_paint(
    target: &mut String,
    defs: &mut String,
    element_id: ElementId,
    style: Option<&ElementStyle>,
    diagnostics: &mut Vec<SvgDiagnostic>,
) {
    match stroke_attributes(element_id, style, diagnostics) {
        Some(attributes) => {
            target.push(' ');
            target.push_str(&attributes);
        }
        None => target.push_str(" stroke=\"none\""),
    }

    let Some(fill) = style.and_then(|style| style.fill.as_ref()) else {
        target.push_str(" fill=\"none\"");
        return;
    };
    let start = resolve_color(fill.color, element_id, diagnostics);
    let Some(gradient) = fill.gradient.as_ref() else {
        write!(target, " {}", paint_attributes("fill", &start))
            .expect("writing flowchart fill into String cannot fail");
        return;
    };

    let end = resolve_color(gradient.end_color, element_id, diagnostics);
    let gradient_id = format!("ddn-flowchart-gradient-{}", element_id.0);
    let (x1, y1, x2, y2) = match gradient.axis {
        GradientAxis::AlongX => ("0%", "0%", "100%", "0%"),
        GradientAxis::AlongY => ("0%", "0%", "0%", "100%"),
    };
    write!(
        defs,
        "<linearGradient id=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">",
        gradient_id, x1, y1, x2, y2,
    )
    .expect("writing flowchart gradient into String cannot fail");
    write_gradient_stop(defs, "0%", &start);
    write_gradient_stop(defs, "100%", &end);
    defs.push_str("</linearGradient>");
    write!(target, " fill=\"url(#{gradient_id})\"")
        .expect("writing flowchart gradient reference into String cannot fail");
}

fn stroke_attributes(
    element_id: ElementId,
    style: Option<&ElementStyle>,
    diagnostics: &mut Vec<SvgDiagnostic>,
) -> Option<String> {
    match style {
        Some(style) => style.stroke.as_ref().map(|stroke| {
            let paint = resolve_color(stroke.color, element_id, diagnostics);
            format!(
                "{} stroke-width=\"{}\"",
                paint_attributes("stroke", &paint),
                num(finite_non_negative(stroke.width_mm, DEFAULT_STROKE_MM)),
            )
        }),
        None => Some("stroke=\"#000000\" stroke-width=\"0.25\"".to_owned()),
    }
}

#[derive(Debug, Clone)]
struct Paint {
    css: String,
    opacity: f64,
}

fn resolve_color(
    color: Color,
    element_id: ElementId,
    diagnostics: &mut Vec<SvgDiagnostic>,
) -> Paint {
    match color {
        Color::Rgba { r, g, b, a } => Paint {
            css: format!("#{r:02x}{g:02x}{b:02x}"),
            opacity: f64::from(a) / 255.0,
        },
        Color::SystemPalette { index } => {
            if !diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic,
                    SvgDiagnostic::SystemPaletteFallback {
                        element_id: existing_id,
                        index: existing_index,
                    } if *existing_id == element_id && *existing_index == index
                )
            }) {
                diagnostics.push(SvgDiagnostic::SystemPaletteFallback { element_id, index });
            }
            Paint {
                css: SYSTEM_PALETTE_FALLBACK.to_owned(),
                opacity: 1.0,
            }
        }
    }
}

fn paint_attributes(name: &str, paint: &Paint) -> String {
    if paint.opacity < 1.0 {
        format!(
            "{name}=\"{}\" {name}-opacity=\"{}\"",
            paint.css,
            num(paint.opacity)
        )
    } else {
        format!("{name}=\"{}\"", paint.css)
    }
}

fn write_gradient_stop(target: &mut String, offset: &str, paint: &Paint) {
    write!(
        target,
        "<stop offset=\"{}\" stop-color=\"{}\"",
        offset, paint.css
    )
    .expect("writing flowchart gradient stop into String cannot fail");
    if paint.opacity < 1.0 {
        write!(target, " stop-opacity=\"{}\"", num(paint.opacity))
            .expect("writing flowchart gradient opacity into String cannot fail");
    }
    target.push_str("/>");
}

fn element_geometry_is_finite(element: &Element) -> bool {
    element.bounds_mm.x.is_finite()
        && element.bounds_mm.y.is_finite()
        && element.bounds_mm.width.is_finite()
        && element.bounds_mm.height.is_finite()
        && element.rotation_deg.is_finite()
}

fn rotation_attribute(element: &Element, bounds: Rect) -> String {
    if element.rotation_deg == 0.0 {
        return String::new();
    }
    format!(
        " transform=\"rotate({} {} {})\"",
        num(element.rotation_deg),
        num(bounds.x + bounds.width / 2.0),
        num(bounds.y + bounds.height / 2.0),
    )
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

fn inject_defs(svg: &mut String, defs: &str) {
    if let Some(end_defs) = svg.find("</defs>") {
        svg.insert_str(end_defs, defs);
        return;
    }
    if let Some(root_end) = svg.find('>') {
        svg.insert_str(root_end + 1, &format!("<defs>{defs}</defs>"));
    }
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

fn finite_non_negative(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        fallback
    }
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
