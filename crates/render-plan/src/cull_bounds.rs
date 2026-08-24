use next_domain::{CurveKind, Element, ElementKind, MarkerStyle, Point, Rect};

use super::rotated_aabb;

const PT_TO_MM: f64 = 25.4 / 72.0;
const FLOWCHART_KEY_PREFIX: &str = "builtin:diagramdesigner-flowchart/";
const LEGACY_LINE_SEGS: usize = 32;
const MM_PER_INCH: f64 = 25.4;
const CATMULL_SEGMENTS_PER_INCH: f64 = 50.0;
const CATMULL_MAX_SEGMENTS: usize = 1000;

/// Conservative renderer-independent culling bounds for one planned element.
///
/// Orthogonal connectors can route outside the rectangle formed by their two
/// endpoints when both ends leave in the same direction. The legacy
/// `TAxisLineObject.DrawShape` clearance is the maximum of marker clearance,
/// corner diameter and ten percent of the perpendicular endpoint separation.
/// Expanding every side by that maximum is intentionally conservative: it keeps
/// both the cold planner and `PreparedPage` from dropping a dogleg that is still
/// visible inside the viewport without storing renderer-specific route state.
///
/// Some public `TFlowchartObject` shapes also draw outside their serialized object
/// rectangle. Slanted shapes extend horizontally by one eighth of the object
/// height, while `foOddRounded2` extends its right-hand curve by half the height.
///
/// Curve bounds follow the public `TCurveLineObject` geometry. Catmull-Rom and
/// legacy spline families are sampled with the same public segment contracts used
/// for rendering; Bezier and line-segment curves stay inside their control-point
/// hull. Any semantic excursion is converted to a symmetric expansion so element
/// rotation continues around the serialized object centre. Standard marker
/// clearance is included when version-aware connector metadata is present.
pub(crate) fn element_cull_bounds(element: &Element) -> Option<Rect> {
    if !rect_is_finite(element.bounds_mm) || !element.rotation_deg.is_finite() {
        return None;
    }

    let mut bounds = normalize_rect(element.bounds_mm);
    if let ElementKind::OrthogonalConnector {
        connector,
        corner_radius_mm,
    } = &element.kind
    {
        let marker_clearance = marker_size_group(connector.start_marker)
            .max(marker_size_group(connector.end_marker))
            * PT_TO_MM
            * 3.0;
        let corner_diameter = finite_non_negative(*corner_radius_mm) * 2.0;
        let perpendicular_clearance = bounds.width.max(bounds.height) / 10.0;
        let clearance = marker_clearance
            .max(corner_diameter)
            .max(perpendicular_clearance);
        bounds = expand_rect(bounds, clearance);
    }

    if let ElementKind::Flowchart { shape_key } = &element.kind {
        match flowchart_code(shape_key) {
            Some(0x31 | 0x32) => {
                let excursion = bounds.height / 8.0;
                bounds.x -= excursion;
                bounds.width += excursion * 2.0;
            }
            Some(0x51) => {
                bounds.width += bounds.height / 2.0;
            }
            _ => {}
        }
    }

    if let ElementKind::Curve {
        curve_kind,
        connector,
        control_points_mm,
    } = &element.kind
    {
        let marker_clearance = connector
            .as_ref()
            .map(|connector| {
                marker_size_group(connector.start_marker)
                    .max(marker_size_group(connector.end_marker))
                    * PT_TO_MM
                    * 3.0
            })
            .unwrap_or(0.0);
        let semantic_excursion = curve_semantic_bounds(*curve_kind, control_points_mm)
            .map(|curve| symmetric_excursion(bounds, curve))
            .unwrap_or(0.0);
        let clearance = marker_clearance.max(semantic_excursion);
        if clearance > 0.0 {
            bounds = expand_rect(bounds, clearance);
        }
    }

    Some(rotated_aabb(bounds, element.rotation_deg))
}

fn flowchart_code(shape_key: &str) -> Option<i32> {
    shape_key
        .strip_prefix(FLOWCHART_KEY_PREFIX)?
        .parse::<i32>()
        .ok()
}

fn marker_size_group(marker: MarkerStyle) -> f64 {
    match marker {
        MarkerStyle::None => 0.0,
        MarkerStyle::Stop => 1.0,
        MarkerStyle::Circle | MarkerStyle::Ball | MarkerStyle::Diamond => 2.0,
        MarkerStyle::Arrow1 | MarkerStyle::Arrow2 | MarkerStyle::Arrow3 => 3.0,
        MarkerStyle::DoubleArrow => 4.0,
        MarkerStyle::UmlIsA | MarkerStyle::UmlHasA => 5.0,
        MarkerStyle::Many => 6.0,
        MarkerStyle::Custom(code) => f64::from((code >> 4) & 0xff),
    }
}

fn curve_semantic_bounds(curve_kind: CurveKind, points: &[Point]) -> Option<Rect> {
    if points.len() < 2 || !points.iter().all(point_is_finite) {
        return None;
    }
    match curve_kind {
        CurveKind::Bezier | CurveKind::LineSegments => bounds_of_points(points),
        CurveKind::CatmullRom => bounds_of_points(&catmull_rom_points(points)?),
        CurveKind::Legacy => bounds_of_points(&legacy_points(points)?),
    }
}

fn catmull_rom_points(points: &[Point]) -> Option<Vec<Point>> {
    if points.len() == 2 {
        return Some(points.to_vec());
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
    let mut result = vec![points[0]];
    let segment_count = points.len() - 1;
    for index in 0..segment_count {
        append_catmull_segment(&mut result, p);
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
    result.iter().all(point_is_finite).then_some(result)
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

fn legacy_points(points: &[Point]) -> Option<Vec<Point>> {
    if points.len() == 2 {
        return Some(points.to_vec());
    }
    let mut sm = [Point::default(); 4];
    for (index, slot) in sm.iter_mut().enumerate() {
        *slot = legacy_point(points, index)?;
    }
    let mut result = vec![sm[0]];
    append_legacy_blend(&mut result, sm, LegacyBlend::First);
    append_legacy_blend(&mut result, sm, LegacyBlend::Center);
    shift_legacy_section(&mut sm);
    if points.len() >= 6 {
        for index in 4..=points.len() - 2 {
            sm[3] = legacy_point(points, index)?;
            append_legacy_blend(&mut result, sm, LegacyBlend::Center);
            shift_legacy_section(&mut sm);
        }
    }
    sm[3] = legacy_point(points, 4usize.max(points.len() - 1))?;
    append_legacy_blend(&mut result, sm, LegacyBlend::Center);
    append_legacy_blend(&mut result, sm, LegacyBlend::Last);
    result.iter().all(point_is_finite).then_some(result)
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
        4 => match index {
            0 | 1 => points.get(index).copied(),
            2 => Some(midpoint(points[1], points[2])),
            _ => points.get(index - 1).copied(),
        },
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

fn bounds_of_points(points: &[Point]) -> Option<Rect> {
    let first = *points.first()?;
    if !point_is_finite(&first) {
        return None;
    }
    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;
    for point in &points[1..] {
        if !point_is_finite(point) {
            return None;
        }
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    Some(Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    })
}

fn symmetric_excursion(serialized: Rect, semantic: Rect) -> f64 {
    let serialized_right = serialized.x + serialized.width;
    let serialized_bottom = serialized.y + serialized.height;
    let semantic_right = semantic.x + semantic.width;
    let semantic_bottom = semantic.y + semantic.height;
    (serialized.x - semantic.x)
        .max(serialized.y - semantic.y)
        .max(semantic_right - serialized_right)
        .max(semantic_bottom - serialized_bottom)
        .max(0.0)
}

fn point_is_finite(point: &Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn rect_is_finite(rect: Rect) -> bool {
    rect.x.is_finite() && rect.y.is_finite() && rect.width.is_finite() && rect.height.is_finite()
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

fn expand_rect(rect: Rect, margin: f64) -> Rect {
    Rect {
        x: rect.x - margin,
        y: rect.y - margin,
        width: rect.width + margin * 2.0,
        height: rect.height + margin * 2.0,
    }
}

#[cfg(test)]
mod tests {
    use next_domain::{AnchorSet, Connector, ElementId, Endpoint, LineStyle};

    use super::*;

    fn orthogonal(marker: MarkerStyle, radius: f64) -> Element {
        Element {
            id: ElementId::new(),
            name: String::new(),
            bounds_mm: Rect {
                x: 100.0,
                y: 100.0,
                width: 20.0,
                height: 10.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::OrthogonalConnector {
                connector: Connector {
                    start: Endpoint {
                        position_mm: Point { x: 100.0, y: 100.0 },
                        connection: None,
                    },
                    end: Endpoint {
                        position_mm: Point { x: 120.0, y: 110.0 },
                        connection: None,
                    },
                    start_marker: marker,
                    end_marker: MarkerStyle::None,
                    line_style: LineStyle::Solid,
                    secondary_color: None,
                },
                corner_radius_mm: radius,
            },
            import: None,
        }
    }

    fn flowchart(code: i32) -> Element {
        Element {
            id: ElementId::new(),
            name: String::new(),
            bounds_mm: Rect {
                x: 100.0,
                y: 100.0,
                width: 20.0,
                height: 40.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::Flowchart {
                shape_key: format!("{FLOWCHART_KEY_PREFIX}{code}"),
            },
            import: None,
        }
    }

    fn catmull_curve(marker: MarkerStyle) -> Element {
        let points = vec![
            Point { x: 100.0, y: 100.0 },
            Point { x: 110.0, y: 110.0 },
            Point { x: 120.0, y: 110.0 },
            Point { x: 130.0, y: 100.0 },
        ];
        Element {
            id: ElementId::new(),
            name: String::new(),
            bounds_mm: Rect {
                x: 100.0,
                y: 100.0,
                width: 30.0,
                height: 10.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::Curve {
                curve_kind: CurveKind::CatmullRom,
                connector: Some(Connector {
                    start: Endpoint {
                        position_mm: points[0],
                        connection: None,
                    },
                    end: Endpoint {
                        position_mm: points[points.len() - 1],
                        connection: None,
                    },
                    start_marker: marker,
                    end_marker: MarkerStyle::None,
                    line_style: LineStyle::Solid,
                    secondary_color: None,
                }),
                control_points_mm: points,
            },
            import: None,
        }
    }

    #[test]
    fn orthogonal_bounds_include_legacy_same_direction_clearance() {
        let bounds = element_cull_bounds(&orthogonal(MarkerStyle::UmlIsA, 1.0)).unwrap();
        let marker_clearance = 5.0 * PT_TO_MM * 3.0;
        assert!((bounds.x - (100.0 - marker_clearance)).abs() < 1e-9);
        assert!((bounds.width - (20.0 + marker_clearance * 2.0)).abs() < 1e-9);
    }

    #[test]
    fn corner_diameter_can_dominate_the_clearance() {
        let bounds = element_cull_bounds(&orthogonal(MarkerStyle::None, 8.0)).unwrap();
        assert_eq!(bounds.x, 84.0);
        assert_eq!(bounds.y, 84.0);
        assert_eq!(bounds.width, 52.0);
        assert_eq!(bounds.height, 42.0);
    }

    #[test]
    fn slanted_flowchart_bounds_include_one_eighth_height_excursion() {
        let bounds = element_cull_bounds(&flowchart(0x31)).unwrap();
        assert_eq!(bounds.x, 95.0);
        assert_eq!(bounds.width, 30.0);
        assert_eq!(bounds.y, 100.0);
        assert_eq!(bounds.height, 40.0);
    }

    #[test]
    fn odd_rounded_2_bounds_include_right_half_height_excursion() {
        let bounds = element_cull_bounds(&flowchart(0x51)).unwrap();
        assert_eq!(bounds.x, 100.0);
        assert_eq!(bounds.width, 40.0);
        assert_eq!(bounds.y, 100.0);
        assert_eq!(bounds.height, 40.0);
    }

    #[test]
    fn catmull_bounds_include_sampled_overshoot_outside_control_bbox() {
        let bounds = element_cull_bounds(&catmull_curve(MarkerStyle::None)).unwrap();
        assert!(bounds.y < 100.0);
        assert!(bounds.y + bounds.height > 110.0);
    }

    #[test]
    fn curve_marker_clearance_expands_all_sides_conservatively() {
        let bounds = element_cull_bounds(&catmull_curve(MarkerStyle::UmlIsA)).unwrap();
        let marker_clearance = 5.0 * PT_TO_MM * 3.0;
        assert!(bounds.x <= 100.0 - marker_clearance + 1e-9);
        assert!(bounds.y <= 100.0 - marker_clearance + 1e-9);
    }
}
