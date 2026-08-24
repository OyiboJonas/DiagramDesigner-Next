use next_domain::{Element, ElementKind, MarkerStyle, Rect};

use super::rotated_aabb;

const PT_TO_MM: f64 = 25.4 / 72.0;
const FLOWCHART_KEY_PREFIX: &str = "builtin:diagramdesigner-flowchart/";

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
    use next_domain::{AnchorSet, Connector, ElementId, Endpoint, LineStyle, Point};

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
}
