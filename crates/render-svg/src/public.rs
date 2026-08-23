//! Public SVG renderer facade.
//!
//! Phase 1 selected the existing SVG renderer as the production backend. Phase 2
//! extends that evidence-tested core in small compatibility layers so each newly
//! supported legacy semantic can be reviewed and tested independently.

#[path = "lib.rs"]
mod core;

pub use core::{SvgDiagnostic, SvgRenderError, SvgRenderOptions, SvgRenderOutput};

use std::fmt::Write as _;

use next_domain::{Color, Document, ElementId, ElementKind, ElementStyle, LineStyle, MarkerStyle};
use render_plan::RenderPlan;

const DEFAULT_STROKE_MM: f64 = 0.25;
const PT_TO_MM: f64 = 25.4 / 72.0;
const SYSTEM_PALETTE_FALLBACK: &str = "#808080";

/// Render a page through the Phase-1 SVG core and apply Phase-2 standard
/// connector-marker semantics.
///
/// Standard marker geometry is normalized from the pinned public Diagram
/// Designer `TBaseConnectorObject.DrawLineEnd` contract. Unknown/custom marker
/// codes remain explicit `ConnectorMarkerDeferred` diagnostics.
pub fn render_plan_to_svg(
    document: &Document,
    page_id: next_domain::PageId,
    plan: &RenderPlan<'_>,
    options: SvgRenderOptions,
) -> Result<SvgRenderOutput, SvgRenderError> {
    let mut output = core::render_plan_to_svg(document, page_id, plan, options)?;
    apply_standard_connector_markers(document, plan, &mut output);
    Ok(output)
}

fn apply_standard_connector_markers(
    document: &Document,
    plan: &RenderPlan<'_>,
    output: &mut SvgRenderOutput,
) {
    let mut defs = String::new();

    for item in &plan.items {
        let ElementKind::StraightConnector { connector } = &item.element.kind else {
            continue;
        };

        let style = item
            .element
            .style_id
            .and_then(|style_id| document.styles.iter().find(|style| style.id == style_id));
        let stroke = connector_stroke(style);
        let secondary = resolve_secondary_paint(
            connector.secondary_color.as_ref(),
            item.element.id,
            &mut output.diagnostics,
        );
        let mut line_attributes = String::new();

        for (slot, marker) in [
            ("start", connector.start_marker),
            ("end", connector.end_marker),
        ] {
            if standard_marker_name(marker).is_none() {
                continue;
            }
            let Some(stroke) = stroke.as_ref() else {
                // An explicit style with no stroke intentionally hides the line and
                // therefore its endpoint decoration as well. The marker semantic is
                // still supported, so no deferred diagnostic is retained.
                continue;
            };

            let marker_id = format!("ddn-marker-{}-{slot}", item.element.id.0);
            defs.push_str(&render_marker_definition(
                &marker_id,
                marker,
                connector.line_style,
                stroke,
                &secondary,
            ));
            write!(line_attributes, " marker-{slot}=\"url(#{marker_id})\"")
                .expect("writing SVG marker attributes into String cannot fail");
        }

        if !line_attributes.is_empty() {
            inject_line_attributes(&mut output.svg, item.element.id, &line_attributes);
        }
    }

    output.diagnostics.retain(|diagnostic| {
        !matches!(
            diagnostic,
            SvgDiagnostic::ConnectorMarkerDeferred { marker, .. }
                if standard_marker_name(*marker).is_some()
        )
    });

    if !defs.is_empty() {
        inject_defs(&mut output.svg, &defs);
    }
}

#[derive(Debug, Clone)]
struct StrokePaint {
    width_mm: f64,
    paint: Paint,
}

fn connector_stroke(style: Option<&ElementStyle>) -> Option<StrokePaint> {
    match style {
        Some(style) => style.stroke.as_ref().map(|stroke| StrokePaint {
            width_mm: finite_non_negative(stroke.width_mm, DEFAULT_STROKE_MM),
            paint: resolve_paint(&stroke.color).0,
        }),
        None => Some(StrokePaint {
            width_mm: DEFAULT_STROKE_MM,
            paint: Paint::opaque("#000000"),
        }),
    }
}

#[derive(Debug, Clone)]
struct Paint {
    css: String,
    opacity: f64,
}

impl Paint {
    fn opaque(css: &str) -> Self {
        Self {
            css: css.to_owned(),
            opacity: 1.0,
        }
    }
}

fn resolve_secondary_paint(
    color: Option<&Color>,
    element_id: ElementId,
    diagnostics: &mut Vec<SvgDiagnostic>,
) -> Paint {
    let Some(color) = color else {
        return Paint::opaque("#ffffff");
    };
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

fn resolve_paint(color: &Color) -> (Paint, Option<u8>) {
    match color {
        Color::Rgba { r, g, b, a } => (
            Paint {
                css: format!("#{r:02x}{g:02x}{b:02x}"),
                opacity: f64::from(*a) / 255.0,
            },
            None,
        ),
        Color::SystemPalette { index } => (Paint::opaque(SYSTEM_PALETTE_FALLBACK), Some(*index)),
    }
}

fn render_marker_definition(
    marker_id: &str,
    marker: MarkerStyle,
    line_style: LineStyle,
    stroke: &StrokePaint,
    secondary: &Paint,
) -> String {
    let name = standard_marker_name(marker).expect("standard marker checked by caller");
    let unit = marker_vector_mm(marker).expect("standard marker has a legacy vector size");
    let radius = 2.0 * stroke.width_mm;
    let reduced_outline_stroke = line_style == LineStyle::Outline
        && matches!(
            marker,
            MarkerStyle::Ball
                | MarkerStyle::Diamond
                | MarkerStyle::Arrow2
                | MarkerStyle::Arrow3
                | MarkerStyle::DoubleArrow
        );
    let marker_stroke_width = if reduced_outline_stroke {
        (stroke.width_mm * 0.25).max(0.05)
    } else {
        stroke.width_mm.max(0.05)
    };
    let use_secondary_fill = matches!(marker, MarkerStyle::UmlIsA | MarkerStyle::UmlHasA)
        || (line_style == LineStyle::Outline
            && matches!(
                marker,
                MarkerStyle::Ball
                    | MarkerStyle::Diamond
                    | MarkerStyle::Arrow2
                    | MarkerStyle::Arrow3
                    | MarkerStyle::DoubleArrow
            ));
    let fill = if use_secondary_fill {
        secondary
    } else {
        &stroke.paint
    };
    let outline_shift = if line_style == LineStyle::Outline
        && matches!(
            marker,
            MarkerStyle::Arrow2 | MarkerStyle::Arrow3 | MarkerStyle::DoubleArrow
        ) {
        stroke.width_mm * 0.9
    } else {
        0.0
    };
    let viewport = (unit * 10.0)
        .max(radius * 4.0)
        .max(marker_stroke_width * 8.0)
        .max(4.0);

    let mut result = format!(
        "<marker id=\"{marker_id}\" data-ddn-marker-style=\"{name}\" markerUnits=\"userSpaceOnUse\" markerWidth=\"{}\" markerHeight=\"{}\" refX=\"0\" refY=\"0\" orient=\"auto-start-reverse\" overflow=\"visible\">",
        num(viewport),
        num(viewport),
    );
    if outline_shift > 0.0 {
        write!(
            result,
            "<g transform=\"translate({} 0)\">",
            num(outline_shift)
        )
        .expect("writing SVG marker transform into String cannot fail");
    }

    let stroke_attrs = paint_attributes("stroke", &stroke.paint);
    let fill_attrs = paint_attributes("fill", fill);
    let common_open = format!(
        "{stroke_attrs} stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" fill=\"none\"",
        num(marker_stroke_width)
    );
    let common_filled = format!(
        "{stroke_attrs} stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" {fill_attrs}",
        num(marker_stroke_width)
    );

    match marker {
        MarkerStyle::Stop => {
            write!(
                result,
                "<path d=\"M 0 {} L 0 {}\" {common_open}/>",
                num(-unit),
                num(unit)
            )
            .unwrap();
        }
        MarkerStyle::Circle => {
            write!(
                result,
                "<circle cx=\"0\" cy=\"0\" r=\"{}\" {common_open}/>",
                num(radius)
            )
            .unwrap();
        }
        MarkerStyle::Ball => {
            write!(
                result,
                "<circle cx=\"0\" cy=\"0\" r=\"{}\" {common_filled}/>",
                num(radius)
            )
            .unwrap();
        }
        MarkerStyle::Diamond | MarkerStyle::UmlHasA => {
            write!(
                result,
                "<path d=\"M 0 0 L {} {} L {} 0 L {} {} Z\" {common_filled}/>",
                num(-unit),
                num(unit),
                num(-2.0 * unit),
                num(-unit),
                num(-unit)
            )
            .unwrap();
        }
        MarkerStyle::Arrow1 => {
            write!(
                result,
                "<path d=\"M {} {} L 0 0 M {} {} L 0 0\" {common_open}/>",
                num(-2.0 * unit),
                num(-unit),
                num(-2.0 * unit),
                num(unit)
            )
            .unwrap();
        }
        MarkerStyle::Arrow2 | MarkerStyle::UmlIsA => {
            write!(
                result,
                "<path d=\"M 0 0 L {} {} L {} {} Z\" {common_filled}/>",
                num(-2.0 * unit),
                num(-unit),
                num(-2.0 * unit),
                num(unit)
            )
            .unwrap();
        }
        MarkerStyle::Arrow3 => {
            write!(
                result,
                "<path d=\"M 0 0 L {} {} L {} 0 L {} {} Z\" {common_filled}/>",
                num(-2.0 * unit),
                num(-unit),
                num(-unit),
                num(-2.0 * unit),
                num(unit)
            )
            .unwrap();
        }
        MarkerStyle::DoubleArrow => {
            write!(
                result,
                "<path d=\"M 0 0 L {} {} L {} {} Z M {} 0 L {} {} L {} {} Z\" {common_filled}/>",
                num(-2.0 * unit),
                num(-unit),
                num(-2.0 * unit),
                num(unit),
                num(-2.0 * unit),
                num(-4.0 * unit),
                num(-unit),
                num(-4.0 * unit),
                num(unit)
            )
            .unwrap();
        }
        MarkerStyle::Many => {
            write!(
                result,
                "<path d=\"M 0 {} L {} 0 M 0 {} L {} 0\" {common_open}/>",
                num(-unit),
                num(-2.0 * unit),
                num(unit),
                num(-2.0 * unit)
            )
            .unwrap();
        }
        MarkerStyle::None | MarkerStyle::Custom(_) => unreachable!("non-standard marker"),
    }

    if outline_shift > 0.0 {
        result.push_str("</g>");
    }
    result.push_str("</marker>");
    result
}

fn paint_attributes(name: &str, paint: &Paint) -> String {
    let mut result = format!("{name}=\"{}\"", paint.css);
    if paint.opacity < 1.0 {
        write!(result, " {name}-opacity=\"{}\"", num(paint.opacity))
            .expect("writing SVG paint opacity into String cannot fail");
    }
    result
}

fn standard_marker_name(marker: MarkerStyle) -> Option<&'static str> {
    match marker {
        MarkerStyle::Stop => Some("stop"),
        MarkerStyle::Circle => Some("circle"),
        MarkerStyle::Ball => Some("ball"),
        MarkerStyle::Diamond => Some("diamond"),
        MarkerStyle::Arrow1 => Some("arrow1"),
        MarkerStyle::Arrow2 => Some("arrow2"),
        MarkerStyle::Arrow3 => Some("arrow3"),
        MarkerStyle::DoubleArrow => Some("double-arrow"),
        MarkerStyle::UmlIsA => Some("uml-is-a"),
        MarkerStyle::UmlHasA => Some("uml-has-a"),
        MarkerStyle::Many => Some("many"),
        MarkerStyle::None | MarkerStyle::Custom(_) => None,
    }
}

fn marker_vector_mm(marker: MarkerStyle) -> Option<f64> {
    let legacy_size_group = match marker {
        MarkerStyle::Stop => 1.0,
        MarkerStyle::Circle | MarkerStyle::Ball | MarkerStyle::Diamond => 2.0,
        MarkerStyle::Arrow1 | MarkerStyle::Arrow2 | MarkerStyle::Arrow3 => 3.0,
        MarkerStyle::DoubleArrow => 4.0,
        MarkerStyle::UmlIsA | MarkerStyle::UmlHasA => 5.0,
        MarkerStyle::Many => 6.0,
        MarkerStyle::None | MarkerStyle::Custom(_) => return None,
    };
    Some(legacy_size_group * PT_TO_MM)
}

fn inject_line_attributes(svg: &mut String, element_id: ElementId, attributes: &str) {
    let needle = format!("<line data-element-id=\"{}\"", element_id.0);
    if let Some(start) = svg.find(&needle) {
        svg.insert_str(start + needle.len(), attributes);
    }
}

fn inject_defs(svg: &mut String, marker_defs: &str) {
    if let Some(end_defs) = svg.find("</defs>") {
        svg.insert_str(end_defs, marker_defs);
        return;
    }
    if let Some(root_end) = svg.find('>') {
        svg.insert_str(root_end + 1, &format!("<defs>{marker_defs}</defs>"));
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
