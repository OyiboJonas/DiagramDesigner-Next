use next_domain::{Point, Rect};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScreenPoint {
    pub x_px: f64,
    pub y_px: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScreenDelta {
    pub x_px: f64,
    pub y_px: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScreenSize {
    pub width_px: f64,
    pub height_px: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportTransform {
    /// Screen-space location of document coordinate (0 mm, 0 mm).
    origin_px: ScreenPoint,
    /// Uniform screen scale. Keeping one scalar prevents accidental non-uniform
    /// document distortion in editor interaction code.
    scale_px_per_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum GeometryError {
    #[error("viewport scale must be finite and greater than zero")]
    InvalidScale,
    #[error("screen size must be finite and non-negative")]
    InvalidScreenSize,
}

impl ViewportTransform {
    pub fn new(origin_px: ScreenPoint, scale_px_per_mm: f64) -> Result<Self, GeometryError> {
        if !scale_px_per_mm.is_finite() || scale_px_per_mm <= 0.0 {
            return Err(GeometryError::InvalidScale);
        }
        Ok(Self {
            origin_px,
            scale_px_per_mm,
        })
    }

    pub fn origin_px(self) -> ScreenPoint {
        self.origin_px
    }

    pub fn scale_px_per_mm(self) -> f64 {
        self.scale_px_per_mm
    }

    pub fn document_to_screen(self, point_mm: Point) -> ScreenPoint {
        ScreenPoint {
            x_px: self.origin_px.x_px + point_mm.x * self.scale_px_per_mm,
            y_px: self.origin_px.y_px + point_mm.y * self.scale_px_per_mm,
        }
    }

    pub fn screen_to_document(self, point_px: ScreenPoint) -> Point {
        Point {
            x: (point_px.x_px - self.origin_px.x_px) / self.scale_px_per_mm,
            y: (point_px.y_px - self.origin_px.y_px) / self.scale_px_per_mm,
        }
    }

    pub fn pan_by(&mut self, delta: ScreenDelta) {
        self.origin_px.x_px += delta.x_px;
        self.origin_px.y_px += delta.y_px;
    }

    /// Change zoom while keeping the document point under `anchor_px` fixed on
    /// screen. This is the invariant required for mouse-wheel / trackpad zoom.
    pub fn zoom_about(
        &mut self,
        anchor_px: ScreenPoint,
        new_scale_px_per_mm: f64,
    ) -> Result<(), GeometryError> {
        if !new_scale_px_per_mm.is_finite() || new_scale_px_per_mm <= 0.0 {
            return Err(GeometryError::InvalidScale);
        }

        let anchor_mm = self.screen_to_document(anchor_px);
        self.scale_px_per_mm = new_scale_px_per_mm;
        self.origin_px = ScreenPoint {
            x_px: anchor_px.x_px - anchor_mm.x * new_scale_px_per_mm,
            y_px: anchor_px.y_px - anchor_mm.y * new_scale_px_per_mm,
        };
        Ok(())
    }

    pub fn visible_document_rect(self, screen_size: ScreenSize) -> Result<Rect, GeometryError> {
        if !screen_size.width_px.is_finite()
            || !screen_size.height_px.is_finite()
            || screen_size.width_px < 0.0
            || screen_size.height_px < 0.0
        {
            return Err(GeometryError::InvalidScreenSize);
        }

        let top_left = self.screen_to_document(ScreenPoint::default());
        let bottom_right = self.screen_to_document(ScreenPoint {
            x_px: screen_size.width_px,
            y_px: screen_size.height_px,
        });
        Ok(normalize_rect(Rect {
            x: top_left.x,
            y: top_left.y,
            width: bottom_right.x - top_left.x,
            height: bottom_right.y - top_left.y,
        }))
    }
}

pub fn normalize_rect(rect: Rect) -> Rect {
    let (x, width) = if rect.width < 0.0 {
        (rect.x + rect.width, -rect.width)
    } else {
        (rect.x, rect.width)
    };
    let (y, height) = if rect.height < 0.0 {
        (rect.y + rect.height, -rect.height)
    } else {
        (rect.y, rect.height)
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

pub fn rect_contains_point(rect: Rect, point: Point) -> bool {
    let rect = normalize_rect(rect);
    point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}

pub fn rects_intersect(left: Rect, right: Rect) -> bool {
    let left = normalize_rect(left);
    let right = normalize_rect(right);
    left.x <= right.x + right.width
        && left.x + left.width >= right.x
        && left.y <= right.y + right.height
        && left.y + left.height >= right.y
}

pub fn rect_center(rect: Rect) -> Point {
    Point {
        x: rect.x + rect.width / 2.0,
        y: rect.y + rect.height / 2.0,
    }
}

pub fn rotate_point_about(point: Point, center: Point, rotation_deg: f64) -> Point {
    let radians = rotation_deg.to_radians();
    let (sin, cos) = radians.sin_cos();
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    Point {
        x: center.x + dx * cos - dy * sin,
        y: center.y + dx * sin + dy * cos,
    }
}

/// Hit-test a rectangular element whose `bounds_mm` describe its unrotated box
/// and whose rotation is around the box center.
pub fn point_in_rotated_rect(point: Point, bounds_mm: Rect, rotation_deg: f64) -> bool {
    let bounds_mm = normalize_rect(bounds_mm);
    let local_point = rotate_point_about(point, rect_center(bounds_mm), -rotation_deg);
    rect_contains_point(bounds_mm, local_point)
}

pub fn distance_to_segment(point: Point, start: Point, end: Point) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return point_distance(point, start);
    }

    let t =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    let projection = Point {
        x: start.x + t * dx,
        y: start.y + t * dy,
    };
    point_distance(point, projection)
}

pub fn point_distance(left: Point, right: Point) -> f64 {
    (left.x - right.x).hypot(left.y - right.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn screen_document_round_trip_is_stable() {
        let viewport = ViewportTransform::new(
            ScreenPoint {
                x_px: 120.0,
                y_px: -40.0,
            },
            3.5,
        )
        .unwrap();
        let document = Point { x: 42.25, y: -8.5 };
        let restored = viewport.screen_to_document(viewport.document_to_screen(document));
        assert_close(restored.x, document.x);
        assert_close(restored.y, document.y);
    }

    #[test]
    fn zoom_keeps_anchor_document_point_fixed() {
        let mut viewport = ViewportTransform::new(
            ScreenPoint {
                x_px: 10.0,
                y_px: 20.0,
            },
            2.0,
        )
        .unwrap();
        let anchor = ScreenPoint {
            x_px: 640.0,
            y_px: 360.0,
        };
        let document_before = viewport.screen_to_document(anchor);
        viewport.zoom_about(anchor, 7.25).unwrap();
        let screen_after = viewport.document_to_screen(document_before);
        assert_close(screen_after.x_px, anchor.x_px);
        assert_close(screen_after.y_px, anchor.y_px);
    }

    #[test]
    fn pan_changes_only_screen_origin() {
        let mut viewport = ViewportTransform::new(
            ScreenPoint {
                x_px: 0.0,
                y_px: 0.0,
            },
            4.0,
        )
        .unwrap();
        viewport.pan_by(ScreenDelta {
            x_px: 15.0,
            y_px: -6.0,
        });
        assert_eq!(
            viewport.origin_px(),
            ScreenPoint {
                x_px: 15.0,
                y_px: -6.0
            }
        );
        assert_eq!(viewport.scale_px_per_mm(), 4.0);
    }

    #[test]
    fn visible_rect_uses_document_units() {
        let viewport = ViewportTransform::new(
            ScreenPoint {
                x_px: -100.0,
                y_px: -50.0,
            },
            10.0,
        )
        .unwrap();
        let visible = viewport
            .visible_document_rect(ScreenSize {
                width_px: 800.0,
                height_px: 600.0,
            })
            .unwrap();
        assert_eq!(
            visible,
            Rect {
                x: 10.0,
                y: 5.0,
                width: 80.0,
                height: 60.0,
            }
        );
    }

    #[test]
    fn rotated_rect_hit_test_uses_inverse_rotation() {
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 4.0,
        };
        assert!(point_in_rotated_rect(
            Point { x: 5.0, y: 6.0 },
            bounds,
            90.0
        ));
        assert!(!point_in_rotated_rect(
            Point { x: 8.5, y: 2.0 },
            bounds,
            90.0
        ));
    }

    #[test]
    fn segment_distance_clamps_to_endpoints() {
        assert_close(
            distance_to_segment(
                Point { x: 4.0, y: 3.0 },
                Point { x: 0.0, y: 0.0 },
                Point { x: 4.0, y: 0.0 },
            ),
            3.0,
        );
        assert_close(
            distance_to_segment(
                Point { x: 7.0, y: 4.0 },
                Point { x: 0.0, y: 0.0 },
                Point { x: 4.0, y: 0.0 },
            ),
            5.0,
        );
    }
}
