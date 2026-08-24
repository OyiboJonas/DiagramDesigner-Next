use std::fmt::Write as _;

use next_domain::{
    Color, Connection, Connector, Document, Element, ElementId, ElementKind, ElementStyle, Endpoint,
    LineStyle, MarkerStyle, Point, Port, Rect,
};
use render_plan::RenderPlan;

use super::{
    DEFAULT_STROKE_MM, PT_TO_MM, Paint, StrokePaint, SvgDiagnostic, SvgRenderOutput,
    connector_stroke, finite_non_negative, num, paint_attributes, resolve_secondary_paint,
    rotation_attribute,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndDirection {
    VerticalTop,
    VerticalBottom,
    HorizontalLeft,
    HorizontalRight,
}

impl EndDirection {
    fn is_vertical(self) -> bool {
        matches!(self, Self::VerticalTop | Self::VerticalBottom)
    }

    fn is_horizontal(self) -> bool {
        matches!(self, Self::HorizontalLeft | Self::HorizontalRight)
    }

    fn name(self) -> &'static str {
        match self {
            Self::VerticalTop => "vertical-top",
            Self::VerticalBottom => "vertical-bottom",
            Self::HorizontalLeft => "horizontal-left",
            Self::HorizontalRight => "horizontal-right",
        }
    }
}

#[derive(Debug, Clone)]
struct OrthogonalRoute {
    points: Vec<Point>,
    path_d: String,
    marker_path_d: String,
}

pub(super) fn apply_orthogonal_connectors(
    document: &Document,
    plan: &RenderPlan<'_>,
    output: &mut SvgRenderOutput,
) {
    let mut supported = Vec::new();

    // Work backwards so every later orthogonal item is already materialized when
    // we search for the next rendered element. Inserting before that element
    // preserves the exact render-plan z-order despite the Phase-1 core having
    // skipped this primitive family.
    for index in (0..plan.items.len()).rev() {
        let item = &plan.items[index];
        let ElementKind::OrthogonalConnector {
            connector,
            corner_radius_mm,
        } = &item.element.kind
        else {
            continue;
        };

        let style = item
            .element
            .style_id
            .and_then(|style_id| document.styles.iter().find(|style| style.id == style_id));
        record_primary_palette_fallback(style, item.element.id, &mut output.diagnostics);
        let stroke = connector_stroke(style);
        let secondary = resolve_secondary_paint(
            connector.secondary_color.as_ref(),
            item.element.id,
            &mut output.diagnostics,
        );
        let directions = resolve_end_directions(document, item.element, connector);
        let route = build_route(connector, directions, *corner_radius_mm);
        let fragment = render_connector_group(
            item.element,
            connector,
            *corner_radius_mm,
            directions,
            &route,
            stroke.as_ref(),
            &secondary,
        );

        if inject_fragment_in_plan_order(&mut output.svg, plan, index, &fragment) {
            supported.push(item.element.id);
            if matches!(connector.line_style, LineStyle::Custom(_)) {
                output
                    .diagnostics
                    .push(SvgDiagnostic::ConnectorLineStyleApproximated {
                        element_id: item.element.id,
                        line_style: connector.line_style,
                    });
            }
            for marker in [connector.start_marker, connector.end_marker] {
                if marker != MarkerStyle::None {
                    output
                        .diagnostics
                        .push(SvgDiagnostic::ConnectorMarkerDeferred {
                            element_id: item.element.id,
                            marker,
                        });
                }
            }
        }
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

fn record_primary_palette_fallback(
    style: Option<&ElementStyle>,
    element_id: ElementId,
    diagnostics: &mut Vec<SvgDiagnostic>,
) {
    let Some(Color::SystemPalette { index }) = style
        .and_then(|style| style.stroke.as_ref())
        .map(|stroke| stroke.color)
    else {
        return;
    };
    if diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic,
            SvgDiagnostic::SystemPaletteFallback {
                element_id: existing_element_id,
                index: existing_index,
            } if *existing_element_id == element_id && *existing_index == index
        )
    }) {
        return;
    }
    diagnostics.push(SvgDiagnostic::SystemPaletteFallback { element_id, index });
}

fn resolve_end_directions(
    document: &Document,
    element: &Element,
    connector: &Connector,
) -> (EndDirection, EndDirection) {
    // Delphi object memory initializes enum fields to the first value. Upstream
    // ResetLinkObjects then calls FindEndDirection(1) followed by (2), and a
    // center-port tie consults the opposite direction. Replaying that sequence
    // makes the otherwise underdetermined center/center case deterministic.
    let provisional_end = EndDirection::VerticalTop;
    let start = resolve_endpoint_direction(
        document,
        element,
        connector,
        &connector.start,
        provisional_end,
    );
    let end = resolve_endpoint_direction(document, element, connector, &connector.end, start);
    (start, end)
}

fn resolve_endpoint_direction(
    document: &Document,
    element: &Element,
    connector: &Connector,
    endpoint: &Endpoint,
    other_direction: EndDirection,
) -> EndDirection {
    let Some(connection) = endpoint.connection else {
        return free_end_direction(connector, endpoint);
    };
    let Some(target) = find_element(document, connection.element_id) else {
        return free_end_direction(connector, endpoint);
    };

    if matches!(target.kind, ElementKind::OrthogonalConnector { .. }) {
        return free_end_direction(connector, endpoint);
    }

    let Some(port) = target.ports.iter().find(|port| port.id == connection.port_id) else {
        return free_end_direction(connector, endpoint);
    };
    connected_port_direction(target, port, other_direction)
}

fn free_end_direction(connector: &Connector, endpoint: &Endpoint) -> EndDirection {
    let start = connector.start.position_mm;
    let end = connector.end.position_mm;
    let midpoint = Point {
        x: (start.x + end.x) / 2.0,
        y: (start.y + end.y) / 2.0,
    };

    if (start.x - end.x).abs() > (start.y - end.y).abs() {
        if endpoint.position_mm.x < midpoint.x {
            EndDirection::HorizontalRight
        } else {
            EndDirection::HorizontalLeft
        }
    } else if endpoint.position_mm.y < midpoint.y {
        EndDirection::VerticalBottom
    } else {
        EndDirection::VerticalTop
    }
}

fn connected_port_direction(
    target: &Element,
    port: &Port,
    other_direction: EndDirection,
) -> EndDirection {
    if port.position.x < 0.05 {
        return EndDirection::HorizontalLeft;
    }
    if port.position.x > 0.95 {
        return EndDirection::HorizontalRight;
    }
    if port.position.y < 0.05 {
        return EndDirection::VerticalTop;
    }
    if port.position.y > 0.95 {
        return EndDirection::VerticalBottom;
    }

    let bounds = normalize_rect(target.bounds_mm);
    let point = Point {
        x: bounds.x + port.position.x * bounds.width,
        y: bounds.y + port.position.y * bounds.height,
    };
    let center = Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    };

    if approx_eq(point.x, center.x) && approx_eq(point.y, center.y) {
        if other_direction.is_vertical() {
            EndDirection::HorizontalLeft
        } else {
            EndDirection::VerticalTop
        }
    } else if (point.x - center.x).abs() > (point.y - center.y).abs() {
        if point.x < center.x {
            EndDirection::HorizontalLeft
        } else {
            EndDirection::HorizontalRight
        }
    } else if point.y < center.y {
        EndDirection::VerticalTop
    } else {
        EndDirection::VerticalBottom
    }
}

fn find_element(document: &Document, element_id: ElementId) -> Option<&Element> {
    document
        .master_layers
        .iter()
        .chain(document.pages.iter().flat_map(|page| page.layers.iter()))
        .flat_map(|layer| layer.scene.elements.iter())
        .find(|element| element.id == element_id)
}

fn build_route(
    connector: &Connector,
    directions: (EndDirection, EndDirection),
    corner_radius_mm: f64,
) -> OrthogonalRoute {
    let start = connector.start.position_mm;
    let end = connector.end.position_mm;
    let (start_direction, end_direction) = directions;
    let radius = finite_non_negative(corner_radius_mm, 0.0);
    let diameter_x = (radius * 2.0).min((end.x - start.x).abs());
    let diameter_y = (radius * 2.0).min((end.y - start.y).abs());
    let marker_clearance = marker_size_group(connector.start_marker)
        .max(marker_size_group(connector.end_marker))
        * PT_TO_MM
        * 3.0;

    let mut points = if start_direction.is_vertical() && end_direction.is_vertical() {
        let center_x = (start.x + end.x) / 2.0;
        let center_y = if start_direction != end_direction {
            (start.y + end.y) / 2.0
        } else {
            let clearance = marker_clearance
                .max(diameter_y)
                .max((start.x - end.x).abs() / 10.0);
            if start_direction == EndDirection::VerticalTop {
                start.y.min(end.y) - clearance
            } else {
                start.y.max(end.y) + clearance
            }
        };
        vec![
            start,
            Point {
                x: start.x,
                y: center_y,
            },
            Point {
                x: center_x,
                y: center_y,
            },
            Point {
                x: end.x,
                y: center_y,
            },
            end,
        ]
    } else if start_direction.is_horizontal() && end_direction.is_horizontal() {
        let center_y = (start.y + end.y) / 2.0;
        let center_x = if start_direction != end_direction {
            (start.x + end.x) / 2.0
        } else {
            let clearance = marker_clearance
                .max(diameter_x)
                .max((start.y - end.y).abs() / 10.0);
            if start_direction == EndDirection::HorizontalLeft {
                start.x.min(end.x) - clearance
            } else {
                start.x.max(end.x) + clearance
            }
        };
        vec![
            start,
            Point {
                x: center_x,
                y: start.y,
            },
            Point {
                x: center_x,
                y: center_y,
            },
            Point {
                x: center_x,
                y: end.y,
            },
            end,
        ]
    } else if start_direction.is_horizontal() {
        vec![
            start,
            Point {
                x: end.x,
                y: start.y,
            },
            end,
        ]
    } else {
        vec![
            start,
            Point {
                x: start.x,
                y: end.y,
            },
            end,
        ]
    };

    simplify_route(&mut points);
    let marker_path_d = sharp_path(&points);
    let use_rounded_corners = matches!(connector.line_style, LineStyle::Solid | LineStyle::Outline)
        && radius > 0.0;
    let path_d = if use_rounded_corners {
        rounded_path(&points, diameter_x / 2.0, diameter_y / 2.0)
    } else {
        marker_path_d.clone()
    };

    OrthogonalRoute {
        points,
        path_d,
        marker_path_d,
    }
}

fn simplify_route(points: &mut Vec<Point>) {
    points.dedup_by(|left, right| approx_eq(left.x, right.x) && approx_eq(left.y, right.y));
    let mut index = 1;
    while index + 1 < points.len() {
        let previous = points[index - 1];
        let current = points[index];
        let next = points[index + 1];
        let collinear_x = approx_eq(previous.x, current.x) && approx_eq(current.x, next.x);
        let collinear_y = approx_eq(previous.y, current.y) && approx_eq(current.y, next.y);
        if collinear_x || collinear_y {
            points.remove(index);
        } else {
            index += 1;
        }
    }
}

fn sharp_path(points: &[Point]) -> String {
    let Some(first) = points.first() else {
        return String::new();
    };
    let mut result = format!("M {} {}", num(first.x), num(first.y));
    for point in &points[1..] {
        write!(result, " L {} {}", num(point.x), num(point.y))
            .expect("writing orthogonal SVG path into String cannot fail");
    }
    result
}

fn rounded_path(points: &[Point], radius_x: f64, radius_y: f64) -> String {
    if points.len() < 3 || radius_x <= 0.0 || radius_y <= 0.0 {
        return sharp_path(points);
    }

    let mut result = format!("M {} {}", num(points[0].x), num(points[0].y));
    for index in 1..points.len() - 1 {
        let previous = points[index - 1];
        let corner = points[index];
        let next = points[index + 1];
        let incoming = unit_axis(previous, corner);
        let outgoing = unit_axis(corner, next);
        if incoming == outgoing || incoming == (-outgoing.0, -outgoing.1) {
            write!(result, " L {} {}", num(corner.x), num(corner.y))
                .expect("writing orthogonal SVG path into String cannot fail");
            continue;
        }

        let incoming_length = axis_distance(previous, corner);
        let outgoing_length = axis_distance(corner, next);
        let incoming_radius = if incoming.0 != 0.0 {
            radius_x.min(incoming_length)
        } else {
            radius_y.min(incoming_length)
        };
        let outgoing_radius = if outgoing.0 != 0.0 {
            radius_x.min(outgoing_length)
        } else {
            radius_y.min(outgoing_length)
        };
        if incoming_radius <= 0.0 || outgoing_radius <= 0.0 {
            write!(result, " L {} {}", num(corner.x), num(corner.y))
                .expect("writing orthogonal SVG path into String cannot fail");
            continue;
        }

        let before = Point {
            x: corner.x - incoming.0 * incoming_radius,
            y: corner.y - incoming.1 * incoming_radius,
        };
        let after = Point {
            x: corner.x + outgoing.0 * outgoing_radius,
            y: corner.y + outgoing.1 * outgoing_radius,
        };
        let arc_rx = if incoming.0 != 0.0 {
            incoming_radius
        } else {
            outgoing_radius
        };
        let arc_ry = if incoming.1 != 0.0 {
            incoming_radius
        } else {
            outgoing_radius
        };
        let cross = incoming.0 * outgoing.1 - incoming.1 * outgoing.0;
        let sweep = if cross > 0.0 { 1 } else { 0 };
        write!(
            result,
            " L {} {} A {} {} 0 0 {} {} {}",
            num(before.x),
            num(before.y),
            num(arc_rx),
            num(arc_ry),
            sweep,
            num(after.x),
            num(after.y),
        )
        .expect("writing rounded orthogonal SVG path into String cannot fail");
    }
    let last = points[points.len() - 1];
    write!(result, " L {} {}", num(last.x), num(last.y))
        .expect("writing orthogonal SVG path into String cannot fail");
    result
}

fn unit_axis(from: Point, to: Point) -> (f64, f64) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if dx.abs() >= dy.abs() && dx != 0.0 {
        (dx.signum(), 0.0)
    } else if dy != 0.0 {
        (0.0, dy.signum())
    } else {
        (0.0, 0.0)
    }
}

fn axis_distance(left: Point, right: Point) -> f64 {
    (right.x - left.x).abs() + (right.y - left.y).abs()
}

fn render_connector_group(
    element: &Element,
    connector: &Connector,
    _corner_radius_mm: f64,
    directions: (EndDirection, EndDirection),
    route: &OrthogonalRoute,
    stroke: Option<&StrokePaint>,
    secondary: &Paint,
) -> String {
    let mut result = String::new();
    write!(
        result,
        "<g data-element-id=\"{}\" data-ddn-orthogonal=\"true\" data-ddn-start-direction=\"{}\" data-ddn-end-direction=\"{}\"{}>",
        element.id.0,
        directions.0.name(),
        directions.1.name(),
        rotation_attribute(element),
    )
    .expect("writing orthogonal SVG group into String cannot fail");

    let Some(stroke) = stroke else {
        write!(
            result,
            "<path data-ddn-connector-outer=\"{}\" d=\"{}\" fill=\"none\" stroke=\"none\"/>",
            element.id.0,
            route.path_d,
        )
        .expect("writing hidden orthogonal connector into String cannot fail");
        result.push_str("</g>");
        return result;
    };

    let stroke_attributes = paint_attributes("stroke", &stroke.paint);
    let stroke_width = stroke.width_mm.max(0.0);
    if let Some(dasharray) = connector_dasharray(connector.line_style, stroke_width) {
        for (segment_index, pair) in route.points.windows(2).enumerate() {
            write!(
                result,
                "<path data-ddn-connector-outer=\"{}\" data-ddn-segment=\"{}\" d=\"M {} {} L {} {}\" fill=\"none\" stroke-linecap=\"round\" {} stroke-width=\"{}\" stroke-dasharray=\"{}\"/>",
                element.id.0,
                segment_index,
                num(pair[0].x),
                num(pair[0].y),
                num(pair[1].x),
                num(pair[1].y),
                stroke_attributes,
                num(stroke_width),
                dasharray,
            )
            .expect("writing styled orthogonal segment into String cannot fail");
        }
    } else {
        write!(
            result,
            "<path data-ddn-connector-outer=\"{}\" d=\"{}\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\" {} stroke-width=\"{}\"/>",
            element.id.0,
            route.path_d,
            stroke_attributes,
            num(stroke_width),
        )
        .expect("writing orthogonal connector into String cannot fail");
    }

    if connector.line_style == LineStyle::Outline {
        write!(
            result,
            "<path data-ddn-outline-inner=\"{}\" d=\"{}\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\" {} stroke-width=\"{}\" pointer-events=\"none\"/>",
            element.id.0,
            route.path_d,
            paint_attributes("stroke", secondary),
            num(stroke_width / 2.0),
        )
        .expect("writing orthogonal outline inner path into String cannot fail");
    }

    if connector.start_marker != MarkerStyle::None || connector.end_marker != MarkerStyle::None {
        write!(
            result,
            "<path data-ddn-marker-target=\"{}\" d=\"{}\" fill=\"none\" stroke=\"none\" pointer-events=\"none\"/>",
            element.id.0,
            route.marker_path_d,
        )
        .expect("writing orthogonal marker carrier into String cannot fail");
    }

    result.push_str("</g>");
    result
}

fn connector_dasharray(line_style: LineStyle, stroke_width: f64) -> Option<String> {
    let w = stroke_width.max(DEFAULT_STROKE_MM).max(0.1);
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

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9
}
