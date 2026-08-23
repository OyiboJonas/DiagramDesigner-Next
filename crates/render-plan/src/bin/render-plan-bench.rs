use std::{env, error::Error, hint::black_box, time::Instant};

use next_domain::{
    AnchorSet, Connector, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,
    ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle, NormalizedPoint,
    Page, PageId, Point, Rect, Scene, Size,
};
use render_plan::{PreparedPage, RenderPlanOptions, build_page_plan};
use uuid::Uuid;

const COLUMNS: usize = 100;
const CELL_WIDTH_MM: f64 = 30.0;
const CELL_HEIGHT_MM: f64 = 22.0;
const VIEWPORT_WIDTH_MM: f64 = 960.0;
const VIEWPORT_HEIGHT_MM: f64 = 540.0;
const ITERATIONS: usize = 120;

fn main() -> Result<(), Box<dyn Error>> {
    let counts = requested_counts()?;
    for count in counts {
        run_benchmark(count)?;
    }
    Ok(())
}

fn requested_counts() -> Result<Vec<usize>, Box<dyn Error>> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    if arguments.is_empty() {
        return Ok(vec![5_000, 20_000]);
    }
    arguments
        .into_iter()
        .map(|argument| argument.parse::<usize>().map_err(Into::into))
        .collect()
}

fn run_benchmark(count: usize) -> Result<(), Box<dyn Error>> {
    if count == 0 {
        return Err("benchmark element count must be greater than zero".into());
    }

    let (document, page_id, world_size) = synthetic_document(count);
    let viewports = benchmark_viewports(world_size);

    let prepare_started = Instant::now();
    let prepared = PreparedPage::build(&document, page_id)?;
    let prepare_elapsed = prepare_started.elapsed();
    let prepared_stats = prepared.stats();

    for viewport in viewports.iter().take(4) {
        let plan = build_page_plan(
            &document,
            page_id,
            RenderPlanOptions {
                viewport_mm: Some(*viewport),
                cull_margin_mm: 12.0,
            },
        )?;
        black_box(plan.items.len());
    }

    let mut timings = Vec::with_capacity(ITERATIONS);
    let mut visible_min = usize::MAX;
    let mut visible_max = 0usize;
    let mut culled_min = usize::MAX;
    let mut culled_max = 0usize;

    for index in 0..ITERATIONS {
        let viewport = viewports[index % viewports.len()];
        let started = Instant::now();
        let plan = build_page_plan(
            &document,
            page_id,
            RenderPlanOptions {
                viewport_mm: Some(viewport),
                cull_margin_mm: 12.0,
            },
        )?;
        let elapsed = started.elapsed();

        if !plan.diagnostics.is_empty() {
            return Err(format!(
                "synthetic benchmark produced diagnostics: {:?}",
                plan.diagnostics
            )
            .into());
        }
        if plan.visited_elements != count {
            return Err(format!(
                "synthetic benchmark visited {} of {count} elements",
                plan.visited_elements
            )
            .into());
        }

        visible_min = visible_min.min(plan.items.len());
        visible_max = visible_max.max(plan.items.len());
        culled_min = culled_min.min(plan.culled_elements);
        culled_max = culled_max.max(plan.culled_elements);
        black_box(plan.items.len());
        timings.push(elapsed);
    }

    timings.sort_unstable();
    let p50 = timings[percentile_index(timings.len(), 50)];
    let p95 = timings[percentile_index(timings.len(), 95)];
    let p99 = timings[percentile_index(timings.len(), 99)];
    let max = *timings.last().expect("benchmark timings are non-empty");

    println!(
        "BENCH render-plan nodes={count} iterations={ITERATIONS} viewport=3840x2160-equivalent visible={visible_min}..{visible_max} culled={culled_min}..{culled_max} p50_us={} p95_us={} p99_us={} max_us={}",
        p50.as_micros(),
        p95.as_micros(),
        p99.as_micros(),
        max.as_micros(),
    );

    for viewport in viewports.iter().take(4) {
        let plan = prepared.query(RenderPlanOptions {
            viewport_mm: Some(*viewport),
            cull_margin_mm: 12.0,
        })?;
        black_box(plan.items.len());
    }

    let mut prepared_timings = Vec::with_capacity(ITERATIONS);
    let mut prepared_visible_min = usize::MAX;
    let mut prepared_visible_max = 0usize;
    let mut prepared_culled_min = usize::MAX;
    let mut prepared_culled_max = 0usize;

    for index in 0..ITERATIONS {
        let viewport = viewports[index % viewports.len()];
        let started = Instant::now();
        let plan = prepared.query(RenderPlanOptions {
            viewport_mm: Some(viewport),
            cull_margin_mm: 12.0,
        })?;
        let elapsed = started.elapsed();

        if !plan.diagnostics.is_empty() {
            return Err(format!(
                "prepared synthetic benchmark produced diagnostics: {:?}",
                plan.diagnostics
            )
            .into());
        }
        if plan.visited_elements != count {
            return Err(format!(
                "prepared synthetic benchmark represents {} of {count} visited elements",
                plan.visited_elements
            )
            .into());
        }

        prepared_visible_min = prepared_visible_min.min(plan.items.len());
        prepared_visible_max = prepared_visible_max.max(plan.items.len());
        prepared_culled_min = prepared_culled_min.min(plan.culled_elements);
        prepared_culled_max = prepared_culled_max.max(plan.culled_elements);
        black_box(plan.items.len());
        prepared_timings.push(elapsed);
    }

    prepared_timings.sort_unstable();
    let prepared_p50 = prepared_timings[percentile_index(prepared_timings.len(), 50)];
    let prepared_p95 = prepared_timings[percentile_index(prepared_timings.len(), 95)];
    let prepared_p99 = prepared_timings[percentile_index(prepared_timings.len(), 99)];
    let prepared_max = *prepared_timings
        .last()
        .expect("prepared benchmark timings are non-empty");

    println!(
        "BENCH render-plan-prepared nodes={count} iterations={ITERATIONS} viewport=3840x2160-equivalent prepare_us={} cells={} globals={} visible={prepared_visible_min}..{prepared_visible_max} culled={prepared_culled_min}..{prepared_culled_max} p50_us={} p95_us={} p99_us={} max_us={}",
        prepare_elapsed.as_micros(),
        prepared_stats.occupied_cells,
        prepared_stats.global_elements,
        prepared_p50.as_micros(),
        prepared_p95.as_micros(),
        prepared_p99.as_micros(),
        prepared_max.as_micros(),
    );

    Ok(())
}

fn percentile_index(len: usize, percentile: usize) -> usize {
    ((len - 1) * percentile / 100).min(len - 1)
}

fn benchmark_viewports(world_size: Size) -> Vec<Rect> {
    let max_x = (world_size.width - VIEWPORT_WIDTH_MM).max(0.0);
    let max_y = (world_size.height - VIEWPORT_HEIGHT_MM).max(0.0);
    (0..60)
        .map(|index| {
            let x_fraction = ((index * 37) % 101) as f64 / 100.0;
            let y_fraction = ((index * 53) % 101) as f64 / 100.0;
            Rect {
                x: max_x * x_fraction,
                y: max_y * y_fraction,
                width: VIEWPORT_WIDTH_MM,
                height: VIEWPORT_HEIGHT_MM,
            }
        })
        .collect()
}

fn synthetic_document(count: usize) -> (Document, PageId, Size) {
    let namespace = Uuid::from_u128(0x4f7808f6_0aca_4d97_88fa_dfe3d1a35eef);
    let page_id = PageId::v5(namespace, "benchmark-page");
    let layer_id = LayerId::v5(namespace, "benchmark-layer");
    let rows = count.div_ceil(COLUMNS);
    let world_size = Size {
        width: COLUMNS as f64 * CELL_WIDTH_MM,
        height: rows as f64 * CELL_HEIGHT_MM,
    };

    let mut elements = Vec::with_capacity(count);
    let mut roots = Vec::with_capacity(count);
    for index in 0..count {
        let element_id = ElementId::v5(namespace, &format!("benchmark-element-{index}"));
        let column = index % COLUMNS;
        let row = index / COLUMNS;
        let x = column as f64 * CELL_WIDTH_MM + 3.0;
        let y = row as f64 * CELL_HEIGHT_MM + 3.0;
        roots.push(element_id);
        elements.push(synthetic_element(element_id, index, x, y));
    }

    let document = Document {
        id: DocumentId::v5(namespace, "benchmark-document"),
        name: format!("Renderer benchmark {count}"),
        defaults: DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Transparent,
        },
        master_layers: Vec::new(),
        pages: vec![Page {
            id: page_id,
            name: "Benchmark".to_owned(),
            size_mm: world_size,
            layers: vec![Layer {
                id: layer_id,
                name: "Synthetic".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene { roots, elements },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };

    (document, page_id, world_size)
}

fn synthetic_element(id: ElementId, index: usize, x: f64, y: f64) -> Element {
    let bounds_mm = Rect {
        x,
        y,
        width: 20.0,
        height: 14.0,
    };
    let kind = match index % 10 {
        0..=3 => ElementKind::Rectangle {
            corner_radius_mm: if index % 2 == 0 { 0.0 } else { 1.5 },
        },
        4 | 5 => ElementKind::Ellipse,
        6 => ElementKind::Text,
        7 => ElementKind::StraightConnector {
            connector: Connector {
                start: Endpoint {
                    position_mm: Point { x, y: y + 7.0 },
                    connection: None,
                },
                end: Endpoint {
                    position_mm: Point {
                        x: x + 20.0,
                        y: y + 7.0,
                    },
                    connection: None,
                },
                start_marker: MarkerStyle::None,
                end_marker: MarkerStyle::Arrow1,
                line_style: LineStyle::Solid,
                secondary_color: None,
            },
        },
        8 => ElementKind::Polygon {
            vertices: vec![
                NormalizedPoint { x: 0.0, y: 1.0 },
                NormalizedPoint { x: 0.5, y: 0.0 },
                NormalizedPoint { x: 1.0, y: 1.0 },
            ],
        },
        _ => ElementKind::Flowchart {
            shape_key: "process".to_owned(),
        },
    };

    Element {
        id,
        name: String::new(),
        bounds_mm,
        rotation_deg: if index % 17 == 0 { 15.0 } else { 0.0 },
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind,
        import: None,
    }
}
