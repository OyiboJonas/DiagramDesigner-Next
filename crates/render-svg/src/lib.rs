use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use next_domain::{
    AssetId, Color, Document, Element, ElementId, ElementKind, ElementStyle, FillStyle,
    GradientAxis, LineStyle, MarkerStyle, PageId, Rect, RichTextToken, ScriptPosition, StyleId,
    TextHorizontalAlignment, TextStyle, TextVerticalAlignment,
};
use render_plan::{RenderPlan, RenderPrimitiveFamily};
use thiserror::Error;

const DEFAULT_STROKE_MM: f64 = 0.25;
const PT_TO_MM: f64 = 25.4 / 72.0;
const SYSTEM_PALETTE_FALLBACK: &str = "#808080";

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SvgRenderOptions {
    /// SVG view box in document millimetres. `None` uses the complete page.
    pub view_box_mm: Option<Rect>,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgDiagnostic {
    UnsupportedPrimitive {
        element_id: ElementId,
        family: RenderPrimitiveFamily,
    },
    MissingTextBlock {
        element_id: ElementId,
    },
    InvalidGeometry {
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
    SystemPaletteFallback {
        element_id: ElementId,
        index: u8,
    },
    ConnectorMarkerDeferred {
        element_id: ElementId,
        marker: MarkerStyle,
    },
    ConnectorLineStyleApproximated {
        element_id: ElementId,
        line_style: LineStyle,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgRenderOutput {
    pub svg: String,
    pub rendered_elements: usize,
    pub skipped_elements: usize,
    pub diagnostics: Vec<SvgDiagnostic>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SvgRenderError {
    #[error("page {0:?} does not exist")]
    PageNotFound(PageId),
    #[error("SVG view box contains invalid geometry")]
    InvalidViewBox,
    #[error("element {element_id:?} references missing style {style_id:?}")]
    MissingStyle {
        element_id: ElementId,
        style_id: StyleId,
    },
}

/// Candidate SVG adapter over the renderer-independent `RenderPlan` boundary.
///
/// This intentionally does not make SVG the final DiagramDesigner Next renderer.
/// Product/editor state must depend on `render-plan`, not on the SVG structure
/// emitted here, so the Windows/WebView2 exit gate in ADR-019 can still replace
/// this adapter without changing the domain or editor command layers.
pub fn render_plan_to_svg(
    document: &Document,
    page_id: PageId,
    plan: &RenderPlan<'_>,
    options: SvgRenderOptions,
) -> Result<SvgRenderOutput, SvgRenderError> {
    let page = document
        .pages
        .iter()
        .find(|page| page.id == page_id)
        .ok_or(SvgRenderError::PageNotFound(page_id))?;

    let view_box = options.view_box_mm.unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        width: page.size_mm.width,
        height: page.size_mm.height,
    });
    let view_box = normalize_finite_rect(view_box).ok_or(SvgRenderError::InvalidViewBox)?;

    let styles: BTreeMap<_, _> = document
        .styles
        .iter()
        .map(|style| (style.id, style))
        .collect();
    let mut context = RenderContext::default();
    let mut defs = String::new();
    let mut body = String::new();
    let mut rendered_elements = 0usize;
    let mut skipped_elements = 0usize;

    for item in &plan.items {
        let element = item.element;
        if !element_geometry_is_finite(element) {
            context.diagnostics.push(SvgDiagnostic::InvalidGeometry {
                element_id: element.id,
            });
            skipped_elements += 1;
            continue;
        }

        let style = resolve_style(element, &styles)?;
        let rendered = match &element.kind {
            ElementKind::Rectangle { corner_radius_mm } => {
                render_rectangle(
                    &mut body,
                    &mut defs,
                    &mut context,
                    element,
                    style,
                    *corner_radius_mm,
                );
                true
            }
            ElementKind::Ellipse => {
                render_ellipse(&mut body, &mut defs, &mut context, element, style);
                true
            }
            ElementKind::Text => {
                render_text(&mut body, &mut context, document, page_id, element, style)
            }
            ElementKind::StraightConnector { connector } => {
                render_straight_connector(&mut body, &mut context, element, style, connector);
                true
            }
            _ => {
                context
                    .diagnostics
                    .push(SvgDiagnostic::UnsupportedPrimitive {
                        element_id: element.id,
                        family: item.family,
                    });
                false
            }
        };

        if rendered {
            rendered_elements += 1;
        } else {
            skipped_elements += 1;
        }
    }

    let mut svg = String::new();
    let label = format!("{} — {}", document.name, page.name);
    write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" role=\"img\" aria-label=\"{}\" viewBox=\"{} {} {} {}\" width=\"{}mm\" height=\"{}mm\" shape-rendering=\"geometricPrecision\">",
        escape_attr(&label),
        num(view_box.x),
        num(view_box.y),
        num(view_box.width),
        num(view_box.height),
        num(view_box.width),
        num(view_box.height),
    )
    .expect("writing SVG into String cannot fail");
    if !defs.is_empty() {
        svg.push_str("<defs>");
        svg.push_str(&defs);
        svg.push_str("</defs>");
    }
    svg.push_str(&body);
    svg.push_str("</svg>");

    Ok(SvgRenderOutput {
        svg,
        rendered_elements,
        skipped_elements,
        diagnostics: context.diagnostics,
    })
}

#[derive(Default)]
struct RenderContext {
    diagnostics: Vec<SvgDiagnostic>,
    system_palette_fallbacks: BTreeSet<(ElementId, u8)>,
}

fn resolve_style<'a>(
    element: &Element,
    styles: &'a BTreeMap<StyleId, &'a ElementStyle>,
) -> Result<Option<&'a ElementStyle>, SvgRenderError> {
    let Some(style_id) = element.style_id else {
        return Ok(None);
    };
    styles
        .get(&style_id)
        .copied()
        .map(Some)
        .ok_or(SvgRenderError::MissingStyle {
            element_id: element.id,
            style_id,
        })
}

fn render_rectangle(
    body: &mut String,
    defs: &mut String,
    context: &mut RenderContext,
    element: &Element,
    style: Option<&ElementStyle>,
    corner_radius_mm: f64,
) {
    let bounds = normalize_rect(element.bounds_mm);
    let radius = if corner_radius_mm.is_finite() {
        corner_radius_mm
            .max(0.0)
            .min(bounds.width.min(bounds.height) / 2.0)
    } else {
        0.0
    };
    write!(
        body,
        "<rect data-element-id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"",
        element.id.0,
        num(bounds.x),
        num(bounds.y),
        num(bounds.width),
        num(bounds.height),
        num(radius),
    )
    .expect("writing SVG into String cannot fail");
    write_shape_paint(body, defs, context, element, style);
    write_rotation(body, element, bounds);
    body.push_str("/>");
}

fn render_ellipse(
    body: &mut String,
    defs: &mut String,
    context: &mut RenderContext,
    element: &Element,
    style: Option<&ElementStyle>,
) {
    let bounds = normalize_rect(element.bounds_mm);
    write!(
        body,
        "<ellipse data-element-id=\"{}\" cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"",
        element.id.0,
        num(bounds.x + bounds.width / 2.0),
        num(bounds.y + bounds.height / 2.0),
        num(bounds.width / 2.0),
        num(bounds.height / 2.0),
    )
    .expect("writing SVG into String cannot fail");
    write_shape_paint(body, defs, context, element, style);
    write_rotation(body, element, bounds);
    body.push_str("/>");
}

fn render_text(
    body: &mut String,
    context: &mut RenderContext,
    document: &Document,
    page_id: PageId,
    element: &Element,
    style: Option<&ElementStyle>,
) -> bool {
    let Some(text) = &element.text else {
        context.diagnostics.push(SvgDiagnostic::MissingTextBlock {
            element_id: element.id,
        });
        return false;
    };
    let page = document
        .pages
        .iter()
        .find(|page| page.id == page_id)
        .expect("page already validated by render_plan_to_svg");
    let page_number = document
        .pages
        .iter()
        .position(|candidate| candidate.id == page_id)
        .map(|index| index + 1)
        .unwrap_or(1);

    let lines = rich_text_lines(
        &text.content.tokens,
        page_number,
        document.pages.len(),
        &page.name,
    );
    let bounds = normalize_rect(element.bounds_mm);
    let margin = text.layout.margin_mm.max(0.0);
    let default_size_pt = document.defaults.font_size_pt.max(1.0);
    let line_height_mm = default_size_pt * PT_TO_MM * 1.2;
    let total_height_mm = line_height_mm * lines.len().max(1) as f64;
    let start_y = match text.layout.vertical {
        TextVerticalAlignment::Top | TextVerticalAlignment::LegacyUnknown(_) => bounds.y + margin,
        TextVerticalAlignment::Center => bounds.y + (bounds.height - total_height_mm) / 2.0,
        TextVerticalAlignment::Bottom => bounds.y + bounds.height - margin - total_height_mm,
    };
    let (x, anchor) = match text.layout.horizontal {
        TextHorizontalAlignment::Left
        | TextHorizontalAlignment::BlockLeft
        | TextHorizontalAlignment::LegacyUnknown(_) => (bounds.x + margin, "start"),
        TextHorizontalAlignment::Right | TextHorizontalAlignment::BlockRight => {
            (bounds.x + bounds.width - margin, "end")
        }
        TextHorizontalAlignment::Center => (bounds.x + bounds.width / 2.0, "middle"),
    };

    write!(
        body,
        "<text data-element-id=\"{}\" text-anchor=\"{}\" dominant-baseline=\"hanging\" font-family=\"{}\" font-size=\"{}pt\"",
        element.id.0,
        anchor,
        escape_attr(&document.defaults.font_family),
        num(default_size_pt),
    )
    .expect("writing SVG into String cannot fail");
    let default_text_color = style.and_then(|style| style.text_color);
    write_color_attr(
        body,
        context,
        element.id,
        "fill",
        default_text_color.unwrap_or(Color::Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }),
    );
    write_rotation(body, element, bounds);
    body.push('>');

    if lines.is_empty() {
        write!(
            body,
            "<tspan x=\"{}\" y=\"{}\"></tspan>",
            num(x),
            num(start_y),
        )
        .expect("writing SVG into String cannot fail");
    } else {
        for (line_index, line) in lines.iter().enumerate() {
            let y = start_y + line_height_mm * line_index as f64;
            write!(body, "<tspan x=\"{}\" y=\"{}\">", num(x), num(y))
                .expect("writing SVG into String cannot fail");
            for run in line {
                body.push_str("<tspan");
                write_text_run_style(body, context, element.id, &run.style, default_text_color);
                body.push('>');
                body.push_str(&escape_text(&run.text));
                body.push_str("</tspan>");
            }
            body.push_str("</tspan>");
        }
    }
    body.push_str("</text>");
    true
}

fn render_straight_connector(
    body: &mut String,
    context: &mut RenderContext,
    element: &Element,
    style: Option<&ElementStyle>,
    connector: &next_domain::Connector,
) {
    write!(
        body,
        "<line data-element-id=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" fill=\"none\" stroke-linecap=\"round\"",
        element.id.0,
        num(connector.start.position_mm.x),
        num(connector.start.position_mm.y),
        num(connector.end.position_mm.x),
        num(connector.end.position_mm.y),
    )
    .expect("writing SVG into String cannot fail");

    let stroke_width = match style.and_then(|style| style.stroke.as_ref()) {
        Some(stroke) => {
            write_color_attr(body, context, element.id, "stroke", stroke.color);
            write!(body, " stroke-width=\"{}\"", num(stroke.width_mm.max(0.0)))
                .expect("writing SVG into String cannot fail");
            Some(stroke.width_mm.max(0.0))
        }
        None if style.is_some() => {
            body.push_str(" stroke=\"none\"");
            None
        }
        None => {
            body.push_str(" stroke=\"#000000\" stroke-width=\"0.25\"");
            Some(DEFAULT_STROKE_MM)
        }
    };

    if let Some(stroke_width) = stroke_width {
        if let Some(dash) =
            line_dasharray(connector.line_style, stroke_width.max(DEFAULT_STROKE_MM))
        {
            write!(body, " stroke-dasharray=\"{}\"", dash)
                .expect("writing SVG into String cannot fail");
        }
    }
    if matches!(
        connector.line_style,
        LineStyle::Outline | LineStyle::Custom(_)
    ) {
        context
            .diagnostics
            .push(SvgDiagnostic::ConnectorLineStyleApproximated {
                element_id: element.id,
                line_style: connector.line_style,
            });
    }
    for marker in [connector.start_marker, connector.end_marker] {
        if marker != MarkerStyle::None {
            context
                .diagnostics
                .push(SvgDiagnostic::ConnectorMarkerDeferred {
                    element_id: element.id,
                    marker,
                });
        }
    }
    write_rotation(body, element, normalize_rect(element.bounds_mm));
    body.push_str("/>");
}

fn write_shape_paint(
    body: &mut String,
    defs: &mut String,
    context: &mut RenderContext,
    element: &Element,
    style: Option<&ElementStyle>,
) {
    match style.and_then(|style| style.stroke.as_ref()) {
        Some(stroke) => {
            write_color_attr(body, context, element.id, "stroke", stroke.color);
            write!(body, " stroke-width=\"{}\"", num(stroke.width_mm.max(0.0)))
                .expect("writing SVG into String cannot fail");
        }
        None if style.is_some() => body.push_str(" stroke=\"none\""),
        None => body.push_str(" stroke=\"#000000\" stroke-width=\"0.25\""),
    }

    match style.and_then(|style| style.fill.as_ref()) {
        Some(fill) => write_fill(body, defs, context, element, fill),
        None => body.push_str(" fill=\"none\""),
    }
}

fn write_fill(
    body: &mut String,
    defs: &mut String,
    context: &mut RenderContext,
    element: &Element,
    fill: &FillStyle,
) {
    let Some(gradient) = &fill.gradient else {
        write_color_attr(body, context, element.id, "fill", fill.color);
        return;
    };

    let gradient_id = format!("gradient-{}", element.id.0);
    let (x1, y1, x2, y2) = match gradient.axis {
        GradientAxis::AlongX => ("0%", "0%", "100%", "0%"),
        GradientAxis::AlongY => ("0%", "0%", "0%", "100%"),
    };
    write!(
        defs,
        "<linearGradient id=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">",
        gradient_id, x1, y1, x2, y2,
    )
    .expect("writing SVG into String cannot fail");
    write_gradient_stop(defs, context, element.id, "0%", fill.color);
    write_gradient_stop(defs, context, element.id, "100%", gradient.end_color);
    defs.push_str("</linearGradient>");
    write!(body, " fill=\"url(#{})\"", gradient_id).expect("writing SVG into String cannot fail");
}

fn write_gradient_stop(
    defs: &mut String,
    context: &mut RenderContext,
    element_id: ElementId,
    offset: &str,
    color: Color,
) {
    let resolved = resolve_color(context, element_id, color);
    write!(
        defs,
        "<stop offset=\"{}\" stop-color=\"{}\"",
        offset, resolved.css,
    )
    .expect("writing SVG into String cannot fail");
    if resolved.opacity < 1.0 {
        write!(defs, " stop-opacity=\"{}\"", num(resolved.opacity))
            .expect("writing SVG into String cannot fail");
    }
    defs.push_str("/>");
}

fn write_text_run_style(
    body: &mut String,
    context: &mut RenderContext,
    element_id: ElementId,
    style: &TextStyle,
    default_text_color: Option<Color>,
) {
    if style.bold {
        body.push_str(" font-weight=\"700\"");
    }
    if style.italic {
        body.push_str(" font-style=\"italic\"");
    }
    if let Some(family) = style.font_family.as_deref() {
        write!(body, " font-family=\"{}\"", escape_attr(family))
            .expect("writing SVG into String cannot fail");
    } else if style.symbol_font {
        body.push_str(" font-family=\"Symbol, serif\"");
    }

    match (style.font_size_pt, style.script) {
        (Some(size), ScriptPosition::Normal) => {
            write!(body, " font-size=\"{}pt\"", size).expect("writing SVG into String cannot fail");
        }
        (Some(size), ScriptPosition::Subscript | ScriptPosition::Superscript) => {
            write!(body, " font-size=\"{}pt\"", num(f64::from(size) * 0.75))
                .expect("writing SVG into String cannot fail");
        }
        (None, ScriptPosition::Subscript | ScriptPosition::Superscript) => {
            body.push_str(" font-size=\"75%\"");
        }
        (None, ScriptPosition::Normal) => {}
    }

    let mut decorations = Vec::new();
    if style.underline {
        decorations.push("underline");
    }
    if style.strikeout {
        decorations.push("line-through");
    }
    if style.overline {
        decorations.push("overline");
    }
    if !decorations.is_empty() {
        write!(body, " text-decoration=\"{}\"", decorations.join(" "))
            .expect("writing SVG into String cannot fail");
    }
    match style.script {
        ScriptPosition::Normal => {}
        ScriptPosition::Subscript => body.push_str(" baseline-shift=\"sub\""),
        ScriptPosition::Superscript => body.push_str(" baseline-shift=\"super\""),
    }
    if let Some(color) = style.color.or(default_text_color) {
        write_color_attr(body, context, element_id, "fill", color);
    }
}

fn write_color_attr(
    target: &mut String,
    context: &mut RenderContext,
    element_id: ElementId,
    name: &str,
    color: Color,
) {
    let resolved = resolve_color(context, element_id, color);
    write!(target, " {}=\"{}\"", name, resolved.css).expect("writing SVG into String cannot fail");
    if resolved.opacity < 1.0 {
        write!(target, " {}-opacity=\"{}\"", name, num(resolved.opacity))
            .expect("writing SVG into String cannot fail");
    }
}

#[derive(Debug, Clone)]
struct ResolvedColor {
    css: String,
    opacity: f64,
}

fn resolve_color(
    context: &mut RenderContext,
    element_id: ElementId,
    color: Color,
) -> ResolvedColor {
    match color {
        Color::Rgba { r, g, b, a } => ResolvedColor {
            css: format!("#{r:02x}{g:02x}{b:02x}"),
            opacity: f64::from(a) / 255.0,
        },
        Color::SystemPalette { index } => {
            if context.system_palette_fallbacks.insert((element_id, index)) {
                context
                    .diagnostics
                    .push(SvgDiagnostic::SystemPaletteFallback { element_id, index });
            }
            ResolvedColor {
                css: SYSTEM_PALETTE_FALLBACK.to_owned(),
                opacity: 1.0,
            }
        }
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
    .expect("writing SVG into String cannot fail");
}

fn line_dasharray(line_style: LineStyle, stroke_width: f64) -> Option<String> {
    let w = stroke_width.max(0.1);
    let multiples: &[f64] = match line_style {
        LineStyle::Solid | LineStyle::Outline | LineStyle::Custom(_) => return None,
        LineStyle::Dotted1 => &[1.0, 2.0],
        LineStyle::Dotted2 => &[1.0, 3.0],
        LineStyle::Short1 => &[4.0, 2.0],
        LineStyle::Short2 => &[4.0, 4.0],
        LineStyle::Long1 => &[8.0, 3.0],
        LineStyle::Long2 => &[12.0, 4.0],
        LineStyle::DashDot1 => &[8.0, 3.0, 1.0, 3.0],
        LineStyle::DashDot2 => &[8.0, 4.0, 1.0, 4.0],
        LineStyle::DashDash => &[6.0, 2.0, 6.0, 4.0],
    };
    Some(
        multiples
            .iter()
            .map(|multiple| num(multiple * w))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[derive(Debug, Clone)]
struct TextRun {
    text: String,
    style: TextStyle,
}

fn rich_text_lines(
    tokens: &[RichTextToken],
    page_number: usize,
    page_count: usize,
    page_name: &str,
) -> Vec<Vec<TextRun>> {
    let mut lines = vec![Vec::new()];
    for token in tokens {
        match token {
            RichTextToken::Text { text, style } => {
                push_text_with_newlines(&mut lines, text, style);
            }
            RichTextToken::NewLine => lines.push(Vec::new()),
            RichTextToken::PageNumber { style } => lines
                .last_mut()
                .expect("rich text always has a current line")
                .push(TextRun {
                    text: page_number.to_string(),
                    style: style.clone(),
                }),
            RichTextToken::PageCount { style } => lines
                .last_mut()
                .expect("rich text always has a current line")
                .push(TextRun {
                    text: page_count.to_string(),
                    style: style.clone(),
                }),
            RichTextToken::PageName { style } => lines
                .last_mut()
                .expect("rich text always has a current line")
                .push(TextRun {
                    text: page_name.to_owned(),
                    style: style.clone(),
                }),
            RichTextToken::SymbolGlyph {
                legacy_glyph,
                style,
            } => lines
                .last_mut()
                .expect("rich text always has a current line")
                .push(TextRun {
                    text: legacy_glyph.to_string(),
                    style: style.clone(),
                }),
        }
    }
    lines
}

fn push_text_with_newlines(lines: &mut Vec<Vec<TextRun>>, text: &str, style: &TextStyle) {
    for (index, part) in text.split('\n').enumerate() {
        if index > 0 {
            lines.push(Vec::new());
        }
        if !part.is_empty() {
            lines
                .last_mut()
                .expect("rich text always has a current line")
                .push(TextRun {
                    text: part.to_owned(),
                    style: style.clone(),
                });
        }
    }
}

fn normalize_finite_rect(rect: Rect) -> Option<Rect> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
        || rect.width == 0.0
        || rect.height == 0.0
    {
        return None;
    }
    Some(normalize_rect(rect))
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

fn element_geometry_is_finite(element: &Element) -> bool {
    element.bounds_mm.x.is_finite()
        && element.bounds_mm.y.is_finite()
        && element.bounds_mm.width.is_finite()
        && element.bounds_mm.height.is_finite()
        && element.rotation_deg.is_finite()
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

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(value: &str) -> String {
    escape_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use next_domain::{
        AnchorSet, Connection, Connector, ConnectorLabelStyle, DocumentDefaults, DocumentId,
        ElementId, Endpoint, FillStyle, Layer, LayerId, LinearGradient, Page, Point,
        RichTextDocument, Scene, Size, StrokeStyle, TextBlock, TextLayout, TextStyle,
    };
    use render_plan::{RenderPlanOptions, build_page_plan};

    use super::*;

    fn defaults() -> DocumentDefaults {
        DocumentDefaults {
            font_family: "Inter".to_owned(),
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

    fn document(elements: Vec<Element>, styles: Vec<ElementStyle>) -> (Document, PageId) {
        let page_id = PageId::new();
        let roots = elements.iter().map(|element| element.id).collect();
        (
            Document {
                id: DocumentId::new(),
                name: "A&B <diagram>".to_owned(),
                defaults: defaults(),
                master_layers: Vec::new(),
                pages: vec![Page {
                    id: page_id,
                    name: "Page \"one\"".to_owned(),
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
                styles,
                assets: Vec::new(),
                import: None,
            },
            page_id,
        )
    }

    #[test]
    fn renders_initial_primitives_in_planned_order() {
        let rectangle_id = ElementId::new();
        let ellipse_id = ElementId::new();
        let text_id = ElementId::new();
        let connector_id = ElementId::new();

        let rectangle = element(
            rectangle_id,
            10.0,
            ElementKind::Rectangle {
                corner_radius_mm: 2.0,
            },
        );
        let ellipse = element(ellipse_id, 40.0, ElementKind::Ellipse);
        let mut text = element(text_id, 70.0, ElementKind::Text);
        text.text = Some(TextBlock {
            content: RichTextDocument {
                tokens: vec![RichTextToken::Text {
                    text: "Hello".to_owned(),
                    style: TextStyle::default(),
                }],
                tail: None,
                diagnostics: Vec::new(),
            },
            layout: TextLayout {
                horizontal: TextHorizontalAlignment::Left,
                vertical: TextVerticalAlignment::Top,
                margin_mm: 1.0,
            },
        });
        let connector = element(
            connector_id,
            100.0,
            ElementKind::StraightConnector {
                connector: Connector {
                    start: Endpoint {
                        position_mm: Point { x: 100.0, y: 10.0 },
                        connection: None,
                    },
                    end: Endpoint {
                        position_mm: Point { x: 120.0, y: 20.0 },
                        connection: None,
                    },
                    start_marker: MarkerStyle::None,
                    end_marker: MarkerStyle::None,
                    line_style: LineStyle::Solid,
                    secondary_color: None,
                },
            },
        );

        let (document, page_id) = document(vec![rectangle, ellipse, text, connector], Vec::new());
        let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
        let output =
            render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

        assert_eq!(output.rendered_elements, 4);
        assert_eq!(output.skipped_elements, 0);
        assert!(output.diagnostics.is_empty());
        let rectangle_pos = output.svg.find(&rectangle_id.0.to_string()).unwrap();
        let ellipse_pos = output.svg.find(&ellipse_id.0.to_string()).unwrap();
        let text_pos = output.svg.find(&text_id.0.to_string()).unwrap();
        let connector_pos = output.svg.find(&connector_id.0.to_string()).unwrap();
        assert!(rectangle_pos < ellipse_pos && ellipse_pos < text_pos && text_pos < connector_pos);
        assert!(output.svg.contains("<rect"));
        assert!(output.svg.contains("<ellipse"));
        assert!(output.svg.contains("<text"));
        assert!(output.svg.contains("<line"));
        assert!(
            output
                .svg
                .contains("A&amp;B &lt;diagram&gt; — Page &quot;one&quot;")
        );
    }

    #[test]
    fn applies_stroke_fill_gradient_text_color_and_dash_style() {
        let style_id = StyleId::new();
        let style = ElementStyle {
            id: style_id,
            stroke: Some(StrokeStyle {
                width_mm: 0.5,
                color: Color::Rgba {
                    r: 10,
                    g: 20,
                    b: 30,
                    a: 128,
                },
            }),
            fill: Some(FillStyle {
                color: Color::Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                gradient: Some(LinearGradient {
                    end_color: Color::Rgba {
                        r: 0,
                        g: 0,
                        b: 255,
                        a: 255,
                    },
                    axis: GradientAxis::AlongX,
                }),
            }),
            text_color: Some(Color::Rgba {
                r: 1,
                g: 2,
                b: 3,
                a: 255,
            }),
        };
        let rectangle_id = ElementId::new();
        let connector_id = ElementId::new();
        let mut rectangle = element(
            rectangle_id,
            10.0,
            ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
        );
        rectangle.style_id = Some(style_id);
        let mut connector = element(
            connector_id,
            40.0,
            ElementKind::StraightConnector {
                connector: Connector {
                    start: Endpoint {
                        position_mm: Point { x: 40.0, y: 10.0 },
                        connection: None,
                    },
                    end: Endpoint {
                        position_mm: Point { x: 60.0, y: 20.0 },
                        connection: None,
                    },
                    start_marker: MarkerStyle::None,
                    end_marker: MarkerStyle::None,
                    line_style: LineStyle::DashDot1,
                    secondary_color: None,
                },
            },
        );
        connector.style_id = Some(style_id);

        let (document, page_id) = document(vec![rectangle, connector], vec![style]);
        let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
        let output =
            render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

        assert!(output.svg.contains("<linearGradient"));
        assert!(output.svg.contains("fill=\"url(#gradient-"));
        assert!(output.svg.contains("stroke=\"#0a141e\""));
        assert!(output.svg.contains("stroke-opacity=\"0.502\""));
        assert!(output.svg.contains("stroke-dasharray=\"4 1.5 0.5 1.5\""));
    }

    #[test]
    fn resolves_page_fields_and_escapes_text_without_executing_tail() {
        let text_id = ElementId::new();
        let mut text = element(text_id, 10.0, ElementKind::Text);
        let style = TextStyle::default();
        text.text = Some(TextBlock {
            content: RichTextDocument {
                tokens: vec![
                    RichTextToken::Text {
                        text: "<&> ".to_owned(),
                        style: style.clone(),
                    },
                    RichTextToken::PageName {
                        style: style.clone(),
                    },
                    RichTextToken::Text {
                        text: " ".to_owned(),
                        style: style.clone(),
                    },
                    RichTextToken::PageNumber {
                        style: style.clone(),
                    },
                    RichTextToken::Text {
                        text: "/".to_owned(),
                        style: style.clone(),
                    },
                    RichTextToken::PageCount { style },
                ],
                tail: Some(next_domain::TextTailDirective {
                    kind: next_domain::TextTailKind::Action,
                    value: "do-not-execute".to_owned(),
                }),
                diagnostics: Vec::new(),
            },
            layout: TextLayout {
                horizontal: TextHorizontalAlignment::Center,
                vertical: TextVerticalAlignment::Center,
                margin_mm: 0.0,
            },
        });
        let (document, page_id) = document(vec![text], Vec::new());
        let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
        let output =
            render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

        assert!(output.svg.contains("&lt;&amp;&gt; "));
        assert!(output.svg.contains("Page \"one\""));
        assert!(output.svg.contains(">1</tspan>"));
        assert!(!output.svg.contains("do-not-execute"));
    }

    #[test]
    fn reports_deferred_connector_markers_and_unsupported_primitives() {
        let connector_id = ElementId::new();
        let polygon_id = ElementId::new();
        let connector = element(
            connector_id,
            10.0,
            ElementKind::StraightConnector {
                connector: Connector {
                    start: Endpoint {
                        position_mm: Point { x: 10.0, y: 10.0 },
                        connection: Some(Connection {
                            element_id: polygon_id,
                            port_id: next_domain::PortId::new(),
                        }),
                    },
                    end: Endpoint {
                        position_mm: Point { x: 20.0, y: 20.0 },
                        connection: None,
                    },
                    start_marker: MarkerStyle::Arrow1,
                    end_marker: MarkerStyle::Diamond,
                    line_style: LineStyle::Outline,
                    secondary_color: Some(Color::Rgba {
                        r: 255,
                        g: 255,
                        b: 255,
                        a: 255,
                    }),
                },
            },
        );
        let polygon = element(
            polygon_id,
            40.0,
            ElementKind::Polygon {
                vertices: vec![
                    next_domain::NormalizedPoint { x: 0.0, y: 0.0 },
                    next_domain::NormalizedPoint { x: 1.0, y: 1.0 },
                ],
            },
        );
        let (document, page_id) = document(vec![connector, polygon], Vec::new());
        // The synthetic connector points at a port not present on the polygon. The
        // render planner is deliberately renderer-oriented and still produces the
        // leaf order; domain validation remains a separate precondition in the app.
        let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
        let output =
            render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

        assert_eq!(output.rendered_elements, 1);
        assert_eq!(output.skipped_elements, 1);
        assert!(output.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            SvgDiagnostic::UnsupportedPrimitive { element_id, .. } if *element_id == polygon_id
        )));
        assert_eq!(
            output
                .diagnostics
                .iter()
                .filter(|diagnostic| matches!(
                    diagnostic,
                    SvgDiagnostic::ConnectorMarkerDeferred { .. }
                ))
                .count(),
            2
        );
        assert!(output.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            SvgDiagnostic::ConnectorLineStyleApproximated { element_id, .. }
                if *element_id == connector_id
        )));
    }

    #[test]
    fn reports_system_palette_fallback_once_per_element_and_index() {
        let style_id = StyleId::new();
        let style = ElementStyle {
            id: style_id,
            stroke: Some(StrokeStyle {
                width_mm: 0.25,
                color: Color::SystemPalette { index: 7 },
            }),
            fill: Some(FillStyle {
                color: Color::SystemPalette { index: 7 },
                gradient: None,
            }),
            text_color: None,
        };
        let element_id = ElementId::new();
        let mut rectangle = element(
            element_id,
            10.0,
            ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
        );
        rectangle.style_id = Some(style_id);
        let (document, page_id) = document(vec![rectangle], vec![style]);
        let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
        let output =
            render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap();

        assert_eq!(
            output
                .diagnostics
                .iter()
                .filter(|diagnostic| matches!(
                    diagnostic,
                    SvgDiagnostic::SystemPaletteFallback { .. }
                ))
                .count(),
            1
        );
        assert!(output.svg.contains(SYSTEM_PALETTE_FALLBACK));
    }

    #[test]
    fn missing_style_fails_instead_of_silently_changing_paint() {
        let missing_style = StyleId::new();
        let element_id = ElementId::new();
        let mut rectangle = element(
            element_id,
            10.0,
            ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
        );
        rectangle.style_id = Some(missing_style);
        let (document, page_id) = document(vec![rectangle], Vec::new());
        let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
        let error =
            render_plan_to_svg(&document, page_id, &plan, SvgRenderOptions::default()).unwrap_err();

        assert_eq!(
            error,
            SvgRenderError::MissingStyle {
                element_id,
                style_id: missing_style,
            }
        );
    }
}
