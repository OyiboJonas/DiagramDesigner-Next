mod cull_bounds;
mod prepared;
pub(crate) use cull_bounds::element_cull_bounds;
pub use prepared::{PreparedPage, PreparedPageOptions, PreparedPageStats};

use std::collections::{BTreeMap, BTreeSet};

use next_domain::{Document, Element, ElementId, ElementKind, Layer, LayerId, PageId, Rect, Scene};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerScope {
    Master,
    Page,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPrimitiveFamily {
    Text,
    Rectangle,
    Ellipse,
    Connector,
    Image,
    Metafile,
    Polygon,
    Flowchart,
    Curve,
    LayerReference,
}

#[derive(Debug, Clone, Copy)]
pub struct PlannedElement<'a> {
    pub layer_id: LayerId,
    pub layer_scope: LayerScope,
    pub family: RenderPrimitiveFamily,
    pub element: &'a Element,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderDiagnostic {
    MissingRoot {
        layer_id: LayerId,
        element_id: ElementId,
    },
    MissingGroupChild {
        group_id: ElementId,
        child_id: ElementId,
    },
    GroupCycle {
        element_id: ElementId,
    },
    DuplicateTraversal {
        element_id: ElementId,
    },
    InvalidGeometry {
        element_id: ElementId,
    },
}

#[derive(Debug, Clone)]
pub struct RenderPlan<'a> {
    /// Back-to-front render order. Master-layer items always precede page-layer items.
    pub items: Vec<PlannedElement<'a>>,
    pub diagnostics: Vec<RenderDiagnostic>,
    pub visited_elements: usize,
    pub culled_elements: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderPlanOptions {
    /// Document-space viewport. `None` disables culling.
    pub viewport_mm: Option<Rect>,
    /// Extra document-space margin around the viewport for near-edge primitives.
    pub cull_margin_mm: f64,
}

impl Default for RenderPlanOptions {
    fn default() -> Self {
        Self {
            viewport_mm: None,
            cull_margin_mm: 0.0,
        }
    }
}

#[derive(Debug, Error)]
pub enum RenderPlanError {
    #[error("page {0:?} does not exist")]
    PageNotFound(PageId),
    #[error("viewport or culling margin contains invalid geometry")]
    InvalidViewport,
    #[error("prepared render-scene spatial cell size must be finite and greater than zero")]
    InvalidSpatialCellSize,
}

pub fn build_page_plan(
    document: &Document,
    page_id: PageId,
    options: RenderPlanOptions,
) -> Result<RenderPlan<'_>, RenderPlanError> {
    validate_options(options)?;
    let page = document
        .pages
        .iter()
        .find(|page| page.id == page_id)
        .ok_or(RenderPlanError::PageNotFound(page_id))?;

    let mut state = PlanState {
        items: Vec::new(),
        diagnostics: Vec::new(),
        visited: BTreeSet::new(),
        recursion_stack: BTreeSet::new(),
        visited_elements: 0,
        culled_elements: 0,
        options,
    };

    for layer in document.master_layers.iter().filter(|layer| layer.visible) {
        state.plan_layer(layer, LayerScope::Master);
    }
    for layer in page.layers.iter().filter(|layer| layer.visible) {
        state.plan_layer(layer, LayerScope::Page);
    }

    Ok(RenderPlan {
        items: state.items,
        diagnostics: state.diagnostics,
        visited_elements: state.visited_elements,
        culled_elements: state.culled_elements,
    })
}

pub(crate) fn validate_options(options: RenderPlanOptions) -> Result<(), RenderPlanError> {
    if !options.cull_margin_mm.is_finite() || options.cull_margin_mm < 0.0 {
        return Err(RenderPlanError::InvalidViewport);
    }
    if let Some(viewport) = options.viewport_mm {
        if !rect_is_finite(viewport) || viewport.width < 0.0 || viewport.height < 0.0 {
            return Err(RenderPlanError::InvalidViewport);
        }
    }
    Ok(())
}

struct PlanState<'a> {
    items: Vec<PlannedElement<'a>>,
    diagnostics: Vec<RenderDiagnostic>,
    visited: BTreeSet<ElementId>,
    recursion_stack: BTreeSet<ElementId>,
    visited_elements: usize,
    culled_elements: usize,
    options: RenderPlanOptions,
}

impl<'a> PlanState<'a> {
    fn plan_layer(&mut self, layer: &'a Layer, scope: LayerScope) {
        let elements: BTreeMap<_, _> = layer
            .scene
            .elements
            .iter()
            .map(|element| (element.id, element))
            .collect();

        for root in &layer.scene.roots {
            if !elements.contains_key(root) {
                self.diagnostics.push(RenderDiagnostic::MissingRoot {
                    layer_id: layer.id,
                    element_id: *root,
                });
                continue;
            }
            self.visit(*root, layer.id, scope, &layer.scene, &elements);
        }
    }

    fn visit(
        &mut self,
        element_id: ElementId,
        layer_id: LayerId,
        scope: LayerScope,
        scene: &'a Scene,
        elements: &BTreeMap<ElementId, &'a Element>,
    ) {
        if self.recursion_stack.contains(&element_id) {
            self.diagnostics
                .push(RenderDiagnostic::GroupCycle { element_id });
            return;
        }
        if !self.visited.insert(element_id) {
            self.diagnostics
                .push(RenderDiagnostic::DuplicateTraversal { element_id });
            return;
        }

        let Some(element) = elements.get(&element_id).copied() else {
            return;
        };
        self.visited_elements += 1;

        if let ElementKind::Group { children } = &element.kind {
            self.recursion_stack.insert(element_id);
            for child in children {
                if !elements.contains_key(child) {
                    self.diagnostics.push(RenderDiagnostic::MissingGroupChild {
                        group_id: element_id,
                        child_id: *child,
                    });
                    continue;
                }
                self.visit(*child, layer_id, scope, scene, elements);
            }
            self.recursion_stack.remove(&element_id);
            return;
        }

        let Some(family) = primitive_family(&element.kind) else {
            return;
        };

        if !element_geometry_is_finite(element) {
            self.diagnostics
                .push(RenderDiagnostic::InvalidGeometry { element_id });
        }

        if self.is_culled(element) {
            self.culled_elements += 1;
            return;
        }

        let _ = scene;
        self.items.push(PlannedElement {
            layer_id,
            layer_scope: scope,
            family,
            element,
        });
    }

    fn is_culled(&self, element: &Element) -> bool {
        let Some(viewport) = self.options.viewport_mm else {
            return false;
        };
        let expanded = expand_rect(viewport, self.options.cull_margin_mm);
        let Some(bounds) = element_cull_bounds(element) else {
            return true;
        };
        !rects_intersect(expanded, bounds)
    }
}

fn primitive_family(kind: &ElementKind) -> Option<RenderPrimitiveFamily> {
    Some(match kind {
        ElementKind::Text => RenderPrimitiveFamily::Text,
        ElementKind::Rectangle { .. } => RenderPrimitiveFamily::Rectangle,
        ElementKind::Ellipse => RenderPrimitiveFamily::Ellipse,
        ElementKind::StraightConnector { .. } | ElementKind::OrthogonalConnector { .. } => {
            RenderPrimitiveFamily::Connector
        }
        ElementKind::Image { .. } => RenderPrimitiveFamily::Image,
        ElementKind::Metafile { .. } => RenderPrimitiveFamily::Metafile,
        ElementKind::Group { .. } => return None,
        ElementKind::Polygon { .. } => RenderPrimitiveFamily::Polygon,
        ElementKind::Flowchart { .. } => RenderPrimitiveFamily::Flowchart,
        ElementKind::Curve { .. } => RenderPrimitiveFamily::Curve,
        ElementKind::LayerReference { .. } => RenderPrimitiveFamily::LayerReference,
    })
}

fn element_geometry_is_finite(element: &Element) -> bool {
    rect_is_finite(element.bounds_mm) && element.rotation_deg.is_finite()
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

pub(crate) fn expand_rect(rect: Rect, margin: f64) -> Rect {
    let rect = normalize_rect(rect);
    Rect {
        x: rect.x - margin,
        y: rect.y - margin,
        width: rect.width + margin * 2.0,
        height: rect.height + margin * 2.0,
    }
}

pub(crate) fn rotated_aabb(rect: Rect, rotation_deg: f64) -> Rect {
    let rect = normalize_rect(rect);
    if !rotation_deg.is_finite() || rotation_deg == 0.0 {
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

pub(crate) fn rects_intersect(left: Rect, right: Rect) -> bool {
    let left = normalize_rect(left);
    let right = normalize_rect(right);
    left.x <= right.x + right.width
        && left.x + left.width >= right.x
        && left.y <= right.y + right.height
        && left.y + left.height >= right.y
}

#[cfg(test)]
mod tests {
    use next_domain::{
        AnchorSet, ConnectorLabelStyle, DocumentDefaults, DocumentId, ElementKind, Layer, Page,
        Scene, Size,
    };

    use super::*;

    fn element(id: ElementId, bounds_mm: Rect, kind: ElementKind) -> Element {
        Element {
            id,
            name: String::new(),
            bounds_mm,
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind,
            import: None,
        }
    }

    fn defaults() -> DocumentDefaults {
        DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Transparent,
        }
    }

    fn document(master: Layer, page_layer: Layer) -> (Document, PageId) {
        let page_id = PageId::new();
        (
            Document {
                id: DocumentId::new(),
                name: "Render plan test".to_owned(),
                defaults: defaults(),
                master_layers: vec![master],
                pages: vec![Page {
                    id: page_id,
                    name: "Page".to_owned(),
                    size_mm: Size {
                        width: 210.0,
                        height: 297.0,
                    },
                    layers: vec![page_layer],
                }],
                styles: Vec::new(),
                assets: Vec::new(),
                import: None,
            },
            page_id,
        )
    }

    fn layer(id: LayerId, elements: Vec<Element>, roots: Vec<ElementId>) -> Layer {
        Layer {
            id,
            name: String::new(),
            visible: true,
            locked: false,
            draw_color: None,
            scene: Scene { roots, elements },
        }
    }

    #[test]
    fn master_items_precede_page_items() {
        let master_id = ElementId::new();
        let page_id_element = ElementId::new();
        let master = layer(
            LayerId::new(),
            vec![element(
                master_id,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                ElementKind::Rectangle {
                    corner_radius_mm: 0.0,
                },
            )],
            vec![master_id],
        );
        let page_layer = layer(
            LayerId::new(),
            vec![element(
                page_id_element,
                Rect {
                    x: 20.0,
                    y: 20.0,
                    width: 10.0,
                    height: 10.0,
                },
                ElementKind::Ellipse,
            )],
            vec![page_id_element],
        );
        let (document, page_id) = document(master, page_layer);
        let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
        assert_eq!(plan.items.len(), 2);
        assert_eq!(plan.items[0].layer_scope, LayerScope::Master);
        assert_eq!(plan.items[0].element.id, master_id);
        assert_eq!(plan.items[1].layer_scope, LayerScope::Page);
        assert_eq!(plan.items[1].element.id, page_id_element);
    }

    #[test]
    fn viewport_culling_keeps_only_intersecting_leaf_elements() {
        let inside = ElementId::new();
        let outside = ElementId::new();
        let master = layer(LayerId::new(), Vec::new(), Vec::new());
        let page_layer = layer(
            LayerId::new(),
            vec![
                element(
                    inside,
                    Rect {
                        x: 5.0,
                        y: 5.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    ElementKind::Ellipse,
                ),
                element(
                    outside,
                    Rect {
                        x: 500.0,
                        y: 500.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    ElementKind::Ellipse,
                ),
            ],
            vec![inside, outside],
        );
        let (document, page_id) = document(master, page_layer);
        let plan = build_page_plan(
            &document,
            page_id,
            RenderPlanOptions {
                viewport_mm: Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                }),
                cull_margin_mm: 0.0,
            },
        )
        .unwrap();
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].element.id, inside);
        assert_eq!(plan.culled_elements, 1);
    }

    #[test]
    fn group_is_not_rendered_but_children_are_expanded_in_order() {
        let group_id = ElementId::new();
        let first = ElementId::new();
        let second = ElementId::new();
        let group = element(
            group_id,
            Rect::default(),
            ElementKind::Group {
                children: vec![first, second],
            },
        );
        let first_element = element(first, Rect::default(), ElementKind::Text);
        let second_element = element(second, Rect::default(), ElementKind::Ellipse);
        let master = layer(LayerId::new(), Vec::new(), Vec::new());
        let page_layer = layer(
            LayerId::new(),
            vec![first_element, group, second_element],
            vec![group_id],
        );
        let (document, page_id) = document(master, page_layer);
        let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
        let ids: Vec<_> = plan.items.iter().map(|item| item.element.id).collect();
        assert_eq!(ids, vec![first, second]);
        assert_eq!(plan.visited_elements, 3);
    }

    #[test]
    fn malformed_group_cycle_is_diagnosed_without_recursing_forever() {
        let first = ElementId::new();
        let second = ElementId::new();
        let first_group = element(
            first,
            Rect::default(),
            ElementKind::Group {
                children: vec![second],
            },
        );
        let second_group = element(
            second,
            Rect::default(),
            ElementKind::Group {
                children: vec![first],
            },
        );
        let master = layer(LayerId::new(), Vec::new(), Vec::new());
        let page_layer = layer(LayerId::new(), vec![first_group, second_group], vec![first]);
        let (document, page_id) = document(master, page_layer);
        let plan = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
        assert!(plan.items.is_empty());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            RenderDiagnostic::GroupCycle { element_id } if *element_id == first
        )));
    }

    #[test]
    fn rotated_bounds_are_used_for_culling() {
        let rotated = ElementId::new();
        let mut rotated_element = element(
            rotated,
            Rect {
                x: 100.0,
                y: 40.0,
                width: 80.0,
                height: 10.0,
            },
            ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
        );
        rotated_element.rotation_deg = 90.0;
        let master = layer(LayerId::new(), Vec::new(), Vec::new());
        let page_layer = layer(LayerId::new(), vec![rotated_element], vec![rotated]);
        let (document, page_id) = document(master, page_layer);
        let plan = build_page_plan(
            &document,
            page_id,
            RenderPlanOptions {
                viewport_mm: Some(Rect {
                    x: 130.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                }),
                cull_margin_mm: 0.0,
            },
        )
        .unwrap();
        assert_eq!(plan.items.len(), 1);
    }
}
