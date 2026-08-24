use next_domain::{Element, ElementKind, MarkerStyle, Rect};

const PT_TO_MM: f64 = 25.4 / 72.0;

/// Conservative renderer-independent culling bounds for one planned element.
///
/// Orthogonal connectors can route outside the rectangle formed by their two
/// endpoints when both ends leave in the same direction. The legacy
/// `TAxisLineObject.DrawShape` clearance is the maximum of marker clearance,
/// corner diameter and ten percent of the perpendicular endpoint separation.
/// Expanding every side by that maximum is intentionally conservative: it keeps
/// both the cold planner and `PreparedPage` from dropping a dogleg that is still
/// visible inside the viewport without storing renderer-specific route state.
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

    Some(rotated_aabb(bounds, element.rotation_deg))
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

fn rotated_aabb(rect: Rect, rotation_deg: f64) -> Rect {
    if rotation_deg == 0.0 {
        return rect;
    }

    let radians = rotation_deg.to_radians();
    let cos = radians.cos().abs();
    let sin = radians.sin().abs();
    let width = rect.width * cos + rect.height * sin;
    let height = rect.width * sin + rect.height * cos;
    let center_x = rect.x + rect.width / 2.0;
    let center_y = rect.y + rect.height / 2.0;
    Rect {
        x: center_x - width / 2.0,
        y: center_y - height / 2.0,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use next_domain::{
        AnchorSet, Connector, ElementId, Endpoint, LineStyle, Point,
    };

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
}
