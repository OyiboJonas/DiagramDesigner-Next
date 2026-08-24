use std::fmt::Write as _;

use next_domain::{
    Color, Connector, CurveKind, Document, Element, ElementId, ElementKind, ElementStyle, LineStyle,
    MarkerStyle, Point, Rect,
};
use render_plan::RenderPlan;

use super::{SvgDiagnostic, SvgRenderOutput};

const DEFAULT_STROKE_MM: f64 = 0.25;
const PT_TO_MM: f64 = 25.4 / 72.0;
const SYSTEM_PALETTE_FALLBACK: &str = "#808080";
const LEGACY_LINE_SEGS: usize = 32;
const MM_PER_INCH: f64 = 25.4;
const CATMULL_SEGMENTS_PER_INCH: f64 = 50.0;
const CATMULL_MAX_SEGMENTS: usize = 1000;

#[derive(Debug, Clone)]
struct CurveGeometry {
    path_d: String,
    start_direction: Point,
    end_direction: Point,
}

pub(super) fn apply_curves(
    document: &Document,
    plan: &RenderPlan<'_>,
    output: &mut SvgRenderOutput,
) {
    let mut understood = Vec::new();
    let mut rendered = Vec::new();
    let mut defs = String::new();

    for index in (0..plan.items.len()).rev() {
        let item = &plan.items[index];
        let ElementKind::Curve {
            curve_kind,
            connector,
            control_points_mm,
        } = &item.element.kind
        else {
            continue;
        };

        understood.push(item.element.id);
        if control_points_mm.len() < 2
            || !element_geometry_is_finite(item.element)
            || !control_points_mm.iter().all(point_is_finite)
        {
            push_invalid_geometry_once(&mut output.diagnostics, item.element.id);
            continue;
        }

        let Some(geometry) = build_curve_geometry(*curve_kind, control_points_mm) else {
            push_invalid_geometry_once(&mut output.diagnostics, item.element.id);
            continue;
        };
        let style = item
            .element
            .style_id
            .and_then(|style_id| document.styles.iter().find(|style| style.id == style_id));
        let fragment = render_curve(
            item.element,
            *curve_kind,
            connector.as_ref(),
            style,
            &geometry,
            &mut defs,
            &mut output.diagnostics,
        );
        if inject_fragment_in_plan_order(&mut output.svg, plan, index, &fragment) {
            rendered.push(item.element.id);
        }
    }

    if !defs.is_empty() {
        inject_defs(&mut output.svg, &defs);
    }

    output.diagnostics.retain(|diagnostic| {
        !matches!(
            diagnostic,
            SvgDiagnostic::UnsupportedPrimitive { element_id, .. }
                if understood.contains(element_id)
        )
    });
    output.rendered_elements += rendered.len();
    output.skipped_elements = output.skipped_elements.saturating_sub(rendered.len());
}

fn render_curve(
    element: &Element,
    curve_kind: CurveKind,
    connector: Option<&Connector>,
    style: Option<&ElementStyle>,
    geometry: &CurveGeometry,
    defs: &mut String,
    diagnostics: &mut Vec<SvgDiagnostic>,
) -> String {
    let kind_key = match curve_kind {
        CurveKind::CatmullRom => "catmull-rom",
        CurveKind::Legacy => "legacy",
        CurveKind::Bezier => "bezier",
        CurveKind::LineSegments => "line-segments",
    };
    let bounds = normalize_rect(element.bounds_mm);
    let rotation = rotation_attribute(element, bounds);
    let mut result = format!(
        "<g data-element-id=\"{}\" data-ddn-curve-kind=\"{}\"{}>",
        element.id.0, kind_key, rotation,
    );

    let stroke = curve_stroke(element.id, style, diagnostics);
    write!(
        result,
        "<path data-ddn-curve-path=\"{}\" d=\"{}\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
        element.id.0, geometry.path_d,
    )
    .expect("writing curve SVG path into String cannot fail");
    match stroke.as_ref() {
        Some(stroke) => {
            write!(
                result,
                " {} stroke-width=\"{}\"",
                paint_attributes("stroke", &stroke.paint),
                num(stroke.width_mm),
            )
            .expect("writing curve stroke into String cannot fail");
        }
        None => result.push_str(" stroke=\"none\""),
    }
    result.push_str("/>");

    if let Some(connector) = connector {
        for marker in [connector.start_marker, connector.end_marker] {
            if matches!(marker, MarkerStyle::Custom(_)) {
                diagnostics.push(SvgDiagnostic::ConnectorMarkerDeferred {
                    element_id: element.id,
                    marker,
                });
            }
        }

        if let Some(stroke) = stroke.as_ref() {
            let secondary = resolve_secondary_paint(
                connector.secondary_color.as_ref(),
                element.id,
                diagnostics,
            );
            if let Some(marker) = standard_marker_name(connector.start_marker) {
                let marker_id = format!("ddn-curve-marker-{}-start", element.id.0);
                defs.push_str(&render_marker_definition(
                    &marker_id,
                    connector.start_marker,
                    connector.line_style,
                    stroke,
                    &secondary,
                ));
                let start = curve_start(control_points(element));
                write!(
                    result,
                    "<line data-ddn-curve-marker-target=\"start\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" fill=\"none\" stroke=\"none\" pointer-events=\"none\" marker-start=\"url(#{})\" data-ddn-marker-style=\"{}\"/>",
                    num(start.x),
                    num(start.y),
                    num(geometry.start_direction.x),
                    num(geometry.start_direction.y),
                    marker_id,
                    marker,
                )
                .expect("writing curve start marker carrier into String cannot fail");
            }
            if let Some(marker) = standard_marker_name(connector.end_marker) {
                let marker_id = format!("ddn-curve-marker-{}-end", element.id.0);
                defs.push_str(&render_marker_definition(
                    &marker_id,
                    connector.end_marker,
                    connector.line_style,
                    stroke,
                    &secondary,
                ));
                let end = curve_end(control_points(element));
                write!(
                    result,
                    "<line data-ddn-curve-marker-target=\"end\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" fill=\"none\" stroke=\"none\" pointer-events=\"none\" marker-end=\"url(#{})\" data-ddn-marker-style=\"{}\"/>",
                    num(geometry.end_direction.x),
                    num(geometry.end_direction.y),
                    num(end.x),
                    num(end.y),
                    marker_id,
                    marker,
                )
                .expect("writing curve end marker carrier into String cannot fail");
            }
        }
    }

    result.push_str("</g>");
    result
}

fn control_points(element: &Element) -> &[Point] {
    match &element.kind {
        ElementKind::Curve {
            control_points_mm, ..
        } => control_points_mm,
        _ => &[],
    }
}

fn curve_start(points: &[Point]) -> Point {
    points[0]
}

fn curve_end(points: &[Point]) -> Point {
    points[points.len() - 1]
}

fn build_curve_geometry(curve_kind: CurveKind, points: &[Point]) -> Option<CurveGeometry> {
    match curve_kind {
        CurveKind::LineSegments => Some(polyline_geometry(points)),
        CurveKind::Bezier => Some(bezier_geometry(points)),
        CurveKind::CatmullRom => catmull_rom_geometry(points),
        CurveKind::Legacy => legacy_geometry(points),
    }
}

fn polyline_geometry(points: &[Point]) -> CurveGeometry {
    CurveGeometry {
        path_d: polyline_path(points),
        start_direction: points[1],
        end_direction: points[points.len() - 2],
    }
}

fn bezier_geometry(points: &[Point]) -> CurveGeometry {
    let mut padded = points.to_vec();
    let segment_count = (points.len() - 1).div_ceil(3);
    let padded_len = segment_count * 3 + 1;
    while padded.len() < padded_len {
        padded.push(*points.last().expect("curve has at least two points"));
    }

    let mut path = format!("M {} {}", num(padded[0].x), num(padded[0].y));
    for chunk in padded[1..].chunks_exact(3) {
        write!(
            path,
            " C {} {},{} {},{} {}",
            num(chunk[0].x),
            num(chunk[0].y),
            num(chunk[1].x),
            num(chunk[1].y),
            num(chunk[2].x),
            num(chunk[2].y),
        )
        .expect("writing Bezier path into String cannot fail");
    }

    CurveGeometry {
        path_d: path,
        start_direction: points[1],
        end_direction: points[points.len() - 2],
    }
}

fn catmull_rom_geometry(points: &[Point]) -> Option<CurveGeometry> {
    if points.len() == 2 {
        return Some(CurveGeometry {
            path_d: polyline_path(points),
            start_direction: points[1],
            end_direction: points[0],
        });
    }

    let closed = points.first() == points.last();
    let mut p = [Point::default(); 4];
    p[1] = points[0];
    p[2] = points[1];
    p[3] = points[2];
    p[0] = if closed {
        points[points.len() - 2]
    } else {
        p[1]
    };

    let start_direction = Point {
        x: p[1].x + catmull_rom_derivative(p[0].x, p[1].x, p[2].x, p[3].x, 0.1),
        y: p[1].y + catmull_rom_derivative(p[0].y, p[1].y, p[2].y, p[3].y, 0.1),
    };
    let mut path_points = vec![points[0]];
    let segment_count = points.len() - 1;

    for index in 0..segment_count {
        append_catmull_segment(&mut path_points, p);
        if index < segment_count - 1 {
            p[0] = p[1];
            p[1] = p[2];
            p[2] = p[3];
            if closed {
                p[3] = points[(index + 3) % (points.len() - 1)];
            } else if index < points.len() - 3 {
                p[3] = points[index + 3];
            }
        }
    }

    let end_direction = Point {
        x: p[2].x - catmull_rom_derivative(p[0].x, p[1].x, p[2].x, p[3].x, 0.9),
        y: p[2].y - catmull_rom_derivative(p[0].y, p[1].y, p[2].y, p[3].y, 0.9),
    };
    if !path_points.iter().all(point_is_finite)
        || !point_is_finite(&start_direction)
        || !point_is_finite(&end_direction)
    {
        return None;
    }

    Some(CurveGeometry {
        path_d: polyline_path(&path_points),
        start_direction,
        end_direction,
    })
}

fn append_catmull_segment(target: &mut Vec<Point>, p: [Point; 4]) {
    let distance = ((p[2].x - p[1].x).powi(2) + (p[2].y - p[1].y).powi(2)).sqrt();
    let line_segments = ((distance / MM_PER_INCH * CATMULL_SEGMENTS_PER_INCH).floor() as usize)
        .min(CATMULL_MAX_SEGMENTS);
    for index in 1..line_segments {
        let t = index as f64 / line_segments as f64;
        target.push(Point {
            x: catmull_rom_poly(p[0].x, p[1].x, p[2].x, p[3].x, t),
            y: catmull_rom_poly(p[0].y, p[1].y, p[2].y, p[3].y, t),
        });
    }
    target.push(p[2]);
}

fn catmull_rom_poly(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
}

fn catmull_rom_derivative(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    0.5 * ((-p0 + p2)
        + 2.0 * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t
        + 3.0 * (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t)
}

fn legacy_geometry(points: &[Point]) -> Option<CurveGeometry> {
    if points.len() == 2 {
        return Some(CurveGeometry {
            path_d: polyline_path(points),
            start_direction: points[1],
            end_direction: points[0],
        });
    }

    let mut sm = [Point::default(); 4];
    for (index, slot) in sm.iter_mut().enumerate() {
        *slot = legacy_point(points, index)?;
    }
    let mut path_points = vec![sm[0]];
    append_legacy_blend(&mut path_points, sm, LegacyBlend::First);
    append_legacy_blend(&mut path_points, sm, LegacyBlend::Center);
    shift_legacy_section(&mut sm);

    if points.len() >= 6 {
        for index in 4..=points.len() - 2 {
            sm[3] = legacy_point(points, index)?;
            append_legacy_blend(&mut path_points, sm, LegacyBlend::Center);
            shift_legacy_section(&mut sm);
        }
    }

    sm[3] = legacy_point(points, 4usize.max(points.len() - 1))?;
    append_legacy_blend(&mut path_points, sm, LegacyBlend::Center);
    append_legacy_blend(&mut path_points, sm, LegacyBlend::Last);

    if !path_points.iter().all(point_is_finite) {
        return None;
    }
    Some(CurveGeometry {
        path_d: polyline_path(&path_points),
        start_direction: points[1],
        end_direction: points[points.len() - 2],
    })
}

#[derive(Debug, Clone, Copy)]
enum LegacyBlend {
    First,
    Center,
    Last,
}

fn append_legacy_blend(target: &mut Vec<Point>, sm: [Point; 4], blend: LegacyBlend) {
    for index in 1..=LEGACY_LINE_SEGS {
        let u = index as f64 / LEGACY_LINE_SEGS as f64;
        let weights = legacy_blend_weights(blend, u);
        let mut point = Point::default();
        for slot in 0..4 {
            point.x += sm[slot].x * weights[slot];
            point.y += sm[slot].y * weights[slot];
        }
        target.push(point);
    }
}

fn legacy_blend_weights(blend: LegacyBlend, u: f64) -> [f64; 4] {
    let v = u - 1.0;
    let w = u + 1.0;
    match blend {
        LegacyBlend::First => [
            -v * (v - 1.0) * (v - 2.0) / 6.0,
            u * (v - 1.0) * (v - 2.0) / 2.0,
            -u * v * (v - 2.0) / 2.0,
            u * v * (v - 1.0) / 6.0,
        ],
        LegacyBlend::Center => [
            -u * v * (u - 2.0) / 6.0,
            w * v * (u - 2.0) / 2.0,
            -w * u * (u - 2.0) / 2.0,
            w * u * v / 6.0,
        ],
        LegacyBlend::Last => [
            -w * u * (w - 2.0) / 6.0,
            (w + 1.0) * u * (w - 2.0) / 2.0,
            -(w + 1.0) * w * (w - 2.0) / 2.0,
            (w + 1.0) * w * u / 6.0,
        ],
    }
}

fn legacy_point(points: &[Point], index: usize) -> Option<Point> {
    match points.len() {
        3 => match index {
            1 => Some(midpoint(points[0], points[1])),
            3 => Some(midpoint(points[1], points[2])),
            _ => points.get(index / 2).copied(),
        },
        4 => {
            if index < 2 {
                points.get(index).copied()
            } else if index > 2 {
                points.get(index - 1).copied()
            } else {
                Some(midpoint(points[1], points[2]))
            }
        }
        _ => points.get(index).copied(),
    }
}

fn shift_legacy_section(sm: &mut [Point; 4]) {
    sm[0] = sm[1];
    sm[1] = sm[2];
    sm[2] = sm[3];
}

fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
    }
}

fn polyline_path(points: &[Point]) -> String {
    let mut path = format!("M {} {}", num(points[0].x), num(points[0].y));
    for point in &points[1..] {
        write!(path, " L {} {}", num(point.x), num(point.y))
            .expect("writing curve polyline into String cannot fail");
    }
    path
}

#[derive(Debug, Clone)]
struct StrokePaint {
    width_mm: f64,
    paint: Paint,
}

fn curve_stroke(
    element_id: ElementId,
    style: Option<&ElementStyle>,
    diagnostics: &mut Vec<SvgDiagnostic>,
) -> Option<StrokePaint> {
    match style {
        Some(style) => style.stroke.as_ref().map(|stroke| StrokePaint {
            width_mm: finite_non_negative(stroke.width_mm, DEFAULT_STROKE_MM),
            paint: resolve_color(stroke.color, element_id, diagnostics),
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
    color
        .copied()
        .map(|color| resolve_color(color, element_id, diagnostics))
        .unwrap_or_else(|| Paint::opaque("#ffffff"))
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
            Paint::opaque(SYSTEM_PALETTE_FALLBACK)
        }
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
        MarkerStyle::Stop => write!(
            result,
            "<path d=\"M 0 {} L 0 {}\" {common_open}/>",
            num(-unit),
            num(unit)
        )
        .unwrap(),
        MarkerStyle::Circle => write!(
            result,
            "<circle cx=\"0\" cy=\"0\" r=\"{}\" {common_open}/>",
            num(radius)
        )
        .unwrap(),
        MarkerStyle::Ball => write!(
            result,
            "<circle cx=\"0\" cy=\"0\" r=\"{}\" {common_filled}/>",
            num(radius)
        )
        .unwrap(),
        MarkerStyle::Diamond | MarkerStyle::UmlHasA => write!(
            result,
            "<path d=\"M 0 0 L {} {} L {} 0 L {} {} Z\" {common_filled}/>",
            num(-unit),
            num(unit),
            num(-2.0 * unit),
            num(-unit),
            num(-unit)
        )
        .unwrap(),
        MarkerStyle::Arrow1 => write!(
            result,
            "<path d=\"M {} {} L 0 0 M {} {} L 0 0\" {common_open}/>",
            num(-2.0 * unit),
            num(-unit),
            num(-2.0 * unit),
            num(unit)
        )
        .unwrap(),
        MarkerStyle::Arrow2 | MarkerStyle::UmlIsA => write!(
            result,
            "<path d=\"M 0 0 L {} {} L {} {} Z\" {common_filled}/>",
            num(-2.0 * unit),
            num(-unit),
            num(-2.0 * unit),
            num(unit)
        )
        .unwrap(),
        MarkerStyle::Arrow3 => write!(
            result,
            "<path d=\"M 0 0 L {} {} L {} 0 L {} {} Z\" {common_filled}/>",
            num(-2.0 * unit),
            num(-unit),
            num(-unit),
            num(-2.0 * unit),
            num(unit)
        )
        .unwrap(),
        MarkerStyle::DoubleArrow => write!(
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
        .unwrap(),
        MarkerStyle::Many => write!(
            result,
            "<path d=\"M 0 {} L {} 0 M 0 {} L {} 0\" {common_open}/>",
            num(-unit),
            num(-2.0 * unit),
            num(unit),
            num(-2.0 * unit)
        )
        .unwrap(),
        MarkerStyle::None | MarkerStyle::Custom(_) => unreachable!("non-standard marker"),
    }

    if outline_shift > 0.0 {
        result.push_str("</g>");
    }
    result.push_str("</marker>");
    result
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

fn push_invalid_geometry_once(diagnostics: &mut Vec<SvgDiagnostic>, element_id: ElementId) {
    if !diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic,
            SvgDiagnostic::InvalidGeometry {
                element_id: existing_id,
            } if *existing_id == element_id
        )
    }) {
        diagnostics.push(SvgDiagnostic::InvalidGeometry { element_id });
    }
}

fn element_geometry_is_finite(element: &Element) -> bool {
    element.bounds_mm.x.is_finite()
        && element.bounds_mm.y.is_finite()
        && element.bounds_mm.width.is_finite()
        && element.bounds_mm.height.is_finite()
        && element.rotation_deg.is_finite()
}

fn point_is_finite(point: &Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
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
