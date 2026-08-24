use std::collections::BTreeMap;

use next_domain::{Document, Element, LayerId, PageId, Rect};

use crate::{
    LayerScope, PlannedElement, RenderDiagnostic, RenderPlan, RenderPlanError, RenderPlanOptions,
    RenderPrimitiveFamily, build_page_plan, element_cull_bounds, expand_rect, rects_intersect,
    validate_options,
};

const DEFAULT_CELL_SIZE_MM: f64 = 128.0;
const MAX_CELLS_PER_ELEMENT: i128 = 1_024;
const MAX_QUERY_CELLS: i128 = 4_096;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedPageOptions {
    /// Spatial-grid cell size in document millimetres.
    ///
    /// This is an indexing parameter only; it never changes render geometry.
    pub cell_size_mm: f64,
}

impl Default for PreparedPageOptions {
    fn default() -> Self {
        Self {
            cell_size_mm: DEFAULT_CELL_SIZE_MM,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedPageStats {
    pub renderable_elements: usize,
    pub occupied_cells: usize,
    pub global_elements: usize,
    pub cell_size_mm: f64,
}

#[derive(Debug, Clone)]
struct PreparedElement {
    layer_id: LayerId,
    layer_scope: LayerScope,
    family: RenderPrimitiveFamily,
    element: Element,
    cull_bounds_mm: Option<Rect>,
}

/// Immutable renderer snapshot for repeated viewport queries.
///
/// Building a prepared page performs the full scene traversal once and clones the
/// renderable leaf elements into a renderer-owned snapshot. Repeated camera queries
/// then use a spatial grid instead of reconstructing scene maps and traversing every
/// document root. A persistent editor mutation invalidates the snapshot and requires
/// a rebuild (incremental snapshot updates can be added later without changing the
/// query contract).
#[derive(Debug, Clone)]
pub struct PreparedPage {
    page_id: PageId,
    entries: Vec<PreparedElement>,
    diagnostics: Vec<RenderDiagnostic>,
    visited_elements: usize,
    spatial: SpatialGrid,
}

impl PreparedPage {
    pub fn build(document: &Document, page_id: PageId) -> Result<Self, RenderPlanError> {
        Self::build_with_options(document, page_id, PreparedPageOptions::default())
    }

    pub fn build_with_options(
        document: &Document,
        page_id: PageId,
        options: PreparedPageOptions,
    ) -> Result<Self, RenderPlanError> {
        if !options.cell_size_mm.is_finite() || options.cell_size_mm <= 0.0 {
            return Err(RenderPlanError::InvalidSpatialCellSize);
        }

        let base = build_page_plan(document, page_id, RenderPlanOptions::default())?;
        let mut spatial = SpatialGrid::new(options.cell_size_mm);
        let mut entries = Vec::with_capacity(base.items.len());

        for planned in base.items {
            let cull_bounds_mm = element_cull_bounds(planned.element);
            let ordinal = entries.len();
            if let Some(bounds) = cull_bounds_mm {
                spatial.insert(ordinal, bounds);
            }
            entries.push(PreparedElement {
                layer_id: planned.layer_id,
                layer_scope: planned.layer_scope,
                family: planned.family,
                element: planned.element.clone(),
                cull_bounds_mm,
            });
        }

        Ok(Self {
            page_id,
            entries,
            diagnostics: base.diagnostics,
            visited_elements: base.visited_elements,
            spatial,
        })
    }

    pub fn page_id(&self) -> PageId {
        self.page_id
    }

    pub fn stats(&self) -> PreparedPageStats {
        PreparedPageStats {
            renderable_elements: self.entries.len(),
            occupied_cells: self.spatial.cells.len(),
            global_elements: self.spatial.global_ordinals.len(),
            cell_size_mm: self.spatial.cell_size_mm,
        }
    }

    pub fn query(&self, options: RenderPlanOptions) -> Result<RenderPlan<'_>, RenderPlanError> {
        validate_options(options)?;

        let Some(viewport) = options.viewport_mm else {
            return Ok(RenderPlan {
                items: self.entries.iter().map(prepared_as_planned).collect(),
                diagnostics: self.diagnostics.clone(),
                visited_elements: self.visited_elements,
                culled_elements: 0,
            });
        };

        let expanded = expand_rect(viewport, options.cull_margin_mm);
        let candidates = self.spatial.query(expanded, self.entries.len());
        let mut items = Vec::with_capacity(candidates.len());

        for ordinal in candidates {
            let entry = &self.entries[ordinal];
            let Some(bounds) = entry.cull_bounds_mm else {
                // The cold planner diagnoses non-finite bounds and culls them when
                // a viewport is active because intersection tests cannot succeed.
                continue;
            };
            if rects_intersect(expanded, bounds) {
                items.push(prepared_as_planned(entry));
            }
        }

        Ok(RenderPlan {
            culled_elements: self.entries.len().saturating_sub(items.len()),
            items,
            diagnostics: self.diagnostics.clone(),
            visited_elements: self.visited_elements,
        })
    }
}

fn prepared_as_planned(entry: &PreparedElement) -> PlannedElement<'_> {
    PlannedElement {
        layer_id: entry.layer_id,
        layer_scope: entry.layer_scope,
        family: entry.family,
        element: &entry.element,
    }
}

#[derive(Debug, Clone, Copy)]
struct CellRange {
    min_x: i64,
    max_x: i64,
    min_y: i64,
    max_y: i64,
}

impl CellRange {
    fn cell_count(self) -> i128 {
        let width = i128::from(self.max_x) - i128::from(self.min_x) + 1;
        let height = i128::from(self.max_y) - i128::from(self.min_y) + 1;
        width.saturating_mul(height)
    }
}

#[derive(Debug, Clone)]
struct SpatialGrid {
    cell_size_mm: f64,
    cells: BTreeMap<(i64, i64), Vec<usize>>,
    /// Very large elements are queried globally instead of being copied into an
    /// unbounded number of grid cells.
    global_ordinals: Vec<usize>,
}

impl SpatialGrid {
    fn new(cell_size_mm: f64) -> Self {
        Self {
            cell_size_mm,
            cells: BTreeMap::new(),
            global_ordinals: Vec::new(),
        }
    }

    fn insert(&mut self, ordinal: usize, bounds: Rect) {
        let range = cell_range(bounds, self.cell_size_mm);
        if range.cell_count() > MAX_CELLS_PER_ELEMENT {
            self.global_ordinals.push(ordinal);
            return;
        }

        for x in range.min_x..=range.max_x {
            for y in range.min_y..=range.max_y {
                self.cells.entry((x, y)).or_default().push(ordinal);
            }
        }
    }

    fn query(&self, bounds: Rect, total_elements: usize) -> Vec<usize> {
        let range = cell_range(bounds, self.cell_size_mm);
        if range.cell_count() > MAX_QUERY_CELLS {
            // A viewport spanning thousands of cells already covers a material
            // portion of the scene. Falling back to all prepared leaves avoids a
            // pathological nested-cell loop while still avoiding scene traversal.
            return (0..total_elements).collect();
        }

        let mut candidates = self.global_ordinals.clone();
        for x in range.min_x..=range.max_x {
            for y in range.min_y..=range.max_y {
                if let Some(ordinals) = self.cells.get(&(x, y)) {
                    candidates.extend_from_slice(ordinals);
                }
            }
        }

        // Cell traversal order is spatial, not draw order. Sorting by the stable
        // prepared ordinal both deduplicates multi-cell elements and restores the
        // exact master/page/group z-order contract.
        candidates.sort_unstable();
        candidates.dedup();
        candidates
    }
}

fn cell_range(bounds: Rect, cell_size_mm: f64) -> CellRange {
    let min_x = bounds.x.min(bounds.x + bounds.width);
    let max_x = bounds.x.max(bounds.x + bounds.width);
    let min_y = bounds.y.min(bounds.y + bounds.height);
    let max_y = bounds.y.max(bounds.y + bounds.height);

    CellRange {
        min_x: cell_coordinate(min_x, cell_size_mm),
        max_x: cell_coordinate(max_x, cell_size_mm),
        min_y: cell_coordinate(min_y, cell_size_mm),
        max_y: cell_coordinate(max_y, cell_size_mm),
    }
}

fn cell_coordinate(value: f64, cell_size_mm: f64) -> i64 {
    (value / cell_size_mm).floor() as i64
}

#[cfg(test)]
mod tests {
    use next_domain::{
        AnchorSet, ConnectorLabelStyle, DocumentDefaults, DocumentId, ElementId, ElementKind,
        Layer, Page, Scene, Size,
    };

    use super::*;

    fn element(id: ElementId, x: f64, y: f64) -> Element {
        Element {
            id,
            name: String::new(),
            bounds_mm: Rect {
                x,
                y,
                width: 20.0,
                height: 14.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
            import: None,
        }
    }

    fn layer(elements: Vec<Element>) -> Layer {
        let roots = elements.iter().map(|element| element.id).collect();
        Layer {
            id: LayerId::new(),
            name: String::new(),
            visible: true,
            locked: false,
            draw_color: None,
            scene: Scene { roots, elements },
        }
    }

    fn document(master: Vec<Element>, page: Vec<Element>) -> (Document, PageId) {
        let page_id = PageId::new();
        (
            Document {
                id: DocumentId::new(),
                name: "Prepared render test".to_owned(),
                defaults: DocumentDefaults {
                    font_family: "Arial".to_owned(),
                    font_size_pt: 10.0,
                    font_style_bits: 0,
                    object_shadows: false,
                    auto_line_break: true,
                    connector_label_style: ConnectorLabelStyle::Transparent,
                },
                master_layers: vec![layer(master)],
                pages: vec![Page {
                    id: page_id,
                    name: "Page".to_owned(),
                    size_mm: Size {
                        width: 1_000.0,
                        height: 1_000.0,
                    },
                    layers: vec![layer(page)],
                }],
                styles: Vec::new(),
                assets: Vec::new(),
                import: None,
            },
            page_id,
        )
    }

    fn ids(plan: &RenderPlan<'_>) -> Vec<ElementId> {
        plan.items.iter().map(|item| item.element.id).collect()
    }

    #[test]
    fn prepared_full_plan_matches_cold_order() {
        let master = ElementId::new();
        let first = ElementId::new();
        let second = ElementId::new();
        let (document, page_id) = document(
            vec![element(master, 500.0, 500.0)],
            vec![element(first, 0.0, 0.0), element(second, 200.0, 200.0)],
        );

        let cold = build_page_plan(&document, page_id, RenderPlanOptions::default()).unwrap();
        let prepared = PreparedPage::build(&document, page_id).unwrap();
        let hot = prepared.query(RenderPlanOptions::default()).unwrap();

        assert_eq!(ids(&cold), ids(&hot));
        assert_eq!(cold.diagnostics, hot.diagnostics);
        assert_eq!(cold.visited_elements, hot.visited_elements);
        assert_eq!(hot.items[0].layer_scope, LayerScope::Master);
    }

    #[test]
    fn prepared_viewport_query_matches_cold_culling_and_order() {
        let master = ElementId::new();
        let inside_first = ElementId::new();
        let outside = ElementId::new();
        let inside_second = ElementId::new();
        let (document, page_id) = document(
            vec![element(master, 10.0, 10.0)],
            vec![
                element(inside_first, 20.0, 20.0),
                element(outside, 700.0, 700.0),
                element(inside_second, 40.0, 40.0),
            ],
        );
        let options = RenderPlanOptions {
            viewport_mm: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            }),
            cull_margin_mm: 12.0,
        };

        let cold = build_page_plan(&document, page_id, options).unwrap();
        let prepared = PreparedPage::build(&document, page_id).unwrap();
        let hot = prepared.query(options).unwrap();

        assert_eq!(ids(&cold), ids(&hot));
        assert_eq!(cold.culled_elements, hot.culled_elements);
        assert_eq!(cold.visited_elements, hot.visited_elements);
    }

    #[test]
    fn large_element_uses_global_bucket_and_remains_queryable() {
        let id = ElementId::new();
        let mut huge = element(id, -10_000.0, -10_000.0);
        huge.bounds_mm.width = 20_000.0;
        huge.bounds_mm.height = 20_000.0;
        let (document, page_id) = document(Vec::new(), vec![huge]);
        let prepared = PreparedPage::build_with_options(
            &document,
            page_id,
            PreparedPageOptions { cell_size_mm: 1.0 },
        )
        .unwrap();

        assert_eq!(prepared.stats().global_elements, 1);
        let plan = prepared
            .query(RenderPlanOptions {
                viewport_mm: Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                }),
                cull_margin_mm: 0.0,
            })
            .unwrap();
        assert_eq!(ids(&plan), vec![id]);
    }

    #[test]
    fn invalid_spatial_cell_size_is_rejected() {
        let (document, page_id) = document(Vec::new(), Vec::new());
        let error = PreparedPage::build_with_options(
            &document,
            page_id,
            PreparedPageOptions { cell_size_mm: 0.0 },
        )
        .unwrap_err();
        assert!(matches!(error, RenderPlanError::InvalidSpatialCellSize));
    }
}
