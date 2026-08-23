use std::{env, error::Error, hint::black_box, time::Duration, time::Instant};

use editor_core::{EditCommand, EditorSession};
use editor_runtime::{EditorRuntime, PreparedPageCacheStats};
use next_domain::{
    AnchorSet, Connector, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,
    ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle, NextArtifact,
    NormalizedPoint, Page, PageId, Point, Rect, Scene, Size,
};

const COLUMNS: usize = 100;
const CELL_WIDTH_MM: f64 = 30.0;
const CELL_HEIGHT_MM: f64 = 22.0;
const CACHE_CAPACITY: usize = 4;
const DEFAULT_REBUILD_SAMPLES: usize = 20;
const DEFAULT_HIT_SAMPLES: usize = 2_000;
const HISTORY_STATES: usize = 3;
const EVICTION_EDITS: usize = CACHE_CAPACITY;
const SCHEMA: &str = "diagramdesigner-next-prepared-cache-v1";

#[derive(Debug, Clone)]
struct Options {
    counts: Vec<usize>,
    rebuild_samples: usize,
    hit_samples: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = parse_options()?;
    println!(
        "BENCH prepared-cache-meta schema={SCHEMA} cache_capacity={CACHE_CAPACITY} rebuild_samples={} hit_samples={}",
        options.rebuild_samples, options.hit_samples
    );

    for count in options.counts {
        run_benchmark(count, options.rebuild_samples, options.hit_samples)?;
    }
    Ok(())
}

fn parse_options() -> Result<Options, Box<dyn Error>> {
    let mut counts = Vec::new();
    let mut rebuild_samples = DEFAULT_REBUILD_SAMPLES;
    let mut hit_samples = DEFAULT_HIT_SAMPLES;
    let mut arguments = env::args().skip(1);

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--rebuild-samples" => {
                let value = arguments
                    .next()
                    .ok_or("--rebuild-samples requires a positive integer")?;
                rebuild_samples = parse_positive(&value, "rebuild sample count")?;
            }
            "--hit-samples" => {
                let value = arguments
                    .next()
                    .ok_or("--hit-samples requires a positive integer")?;
                hit_samples = parse_positive(&value, "hit sample count")?;
            }
            value if value.starts_with('-') => {
                return Err(format!("unsupported argument: {value}").into());
            }
            value => counts.push(parse_positive(value, "element count")?),
        }
    }

    if counts.is_empty() {
        counts = vec![5_000, 20_000];
    }

    Ok(Options {
        counts,
        rebuild_samples,
        hit_samples,
    })
}

fn parse_positive(value: &str, label: &str) -> Result<usize, Box<dyn Error>> {
    let parsed = value.parse::<usize>()?;
    if parsed == 0 {
        return Err(format!("{label} must be greater than zero").into());
    }
    Ok(parsed)
}

fn run_benchmark(
    count: usize,
    rebuild_samples: usize,
    hit_samples: usize,
) -> Result<(), Box<dyn Error>> {
    let (session, page_id, movable_element_id) = synthetic_session(count)?;
    let rebuild = measure_rebuilds(&session, page_id, rebuild_samples)?;
    let same_state = measure_same_state_hits(&session, page_id, hit_samples)?;
    let history = measure_history_reuse(&session, page_id, movable_element_id)?;
    let eviction = measure_eviction_rebuild(&session, page_id, movable_element_id)?;

    println!(
        "BENCH prepared-cache nodes={count} rebuild_samples={rebuild_samples} rebuild_p50_us={} rebuild_p95_us={} rebuild_p99_us={} rebuild_max_us={} hit_samples={hit_samples} hit_p50_ns={} hit_p95_ns={} hit_p99_ns={} hit_max_ns={} history_hits={} history_p50_ns={} history_p95_ns={} history_max_ns={} history_builds={} eviction_rebuild_us={} eviction_builds={} evictions={}",
        rebuild.p50.as_micros(),
        rebuild.p95.as_micros(),
        rebuild.p99.as_micros(),
        rebuild.max.as_micros(),
        same_state.p50.as_nanos(),
        same_state.p95.as_nanos(),
        same_state.p99.as_nanos(),
        same_state.max.as_nanos(),
        history.samples,
        history.timings.p50.as_nanos(),
        history.timings.p95.as_nanos(),
        history.timings.max.as_nanos(),
        history.stats.builds,
        eviction.elapsed.as_micros(),
        eviction.stats.builds,
        eviction.stats.evictions,
    );

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct TimingSummary {
    p50: Duration,
    p95: Duration,
    p99: Duration,
    max: Duration,
}

#[derive(Debug, Clone, Copy)]
struct HistoryMeasurement {
    samples: usize,
    timings: TimingSummary,
    stats: PreparedPageCacheStats,
}

#[derive(Debug, Clone, Copy)]
struct EvictionMeasurement {
    elapsed: Duration,
    stats: PreparedPageCacheStats,
}

fn measure_rebuilds(
    session: &EditorSession,
    page_id: PageId,
    samples: usize,
) -> Result<TimingSummary, Box<dyn Error>> {
    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        // Session cloning/runtime construction deliberately stay outside the timed
        // region. The measurement answers how expensive one cache miss plus the
        // real PreparedPage::build path is for an already-live editor session.
        let mut runtime = EditorRuntime::with_cache_capacity(session.clone(), CACHE_CAPACITY)?;
        let started = Instant::now();
        let occupied_cells = runtime.prepared_page(page_id)?.stats().occupied_cells;
        timings.push(started.elapsed());
        black_box(occupied_cells);
        require_stats(
            runtime.prepared_page_cache_stats(),
            PreparedPageCacheStats {
                capacity: CACHE_CAPACITY,
                entries: 1,
                hits: 0,
                misses: 1,
                builds: 1,
                evictions: 0,
            },
            "isolated rebuild",
        )?;
    }
    Ok(summarize(timings))
}

fn measure_same_state_hits(
    session: &EditorSession,
    page_id: PageId,
    samples: usize,
) -> Result<TimingSummary, Box<dyn Error>> {
    let mut runtime = EditorRuntime::with_cache_capacity(session.clone(), CACHE_CAPACITY)?;
    black_box(runtime.prepared_page(page_id)?.stats().occupied_cells);

    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        black_box(runtime.prepared_page(page_id)?.stats().occupied_cells);
        timings.push(started.elapsed());
    }

    require_stats(
        runtime.prepared_page_cache_stats(),
        PreparedPageCacheStats {
            capacity: CACHE_CAPACITY,
            entries: 1,
            hits: samples as u64,
            misses: 1,
            builds: 1,
            evictions: 0,
        },
        "same-state cache hit",
    )?;
    Ok(summarize(timings))
}

fn measure_history_reuse(
    session: &EditorSession,
    page_id: PageId,
    element_id: ElementId,
) -> Result<HistoryMeasurement, Box<dyn Error>> {
    let mut runtime = EditorRuntime::with_cache_capacity(session.clone(), CACHE_CAPACITY)?;
    black_box(runtime.prepared_page(page_id)?.stats().occupied_cells);

    for step in 0..HISTORY_STATES {
        move_element(&mut runtime, element_id, step + 1)?;
        black_box(runtime.prepared_page(page_id)?.stats().occupied_cells);
    }

    let before = runtime.prepared_page_cache_stats();
    if before.builds != (HISTORY_STATES + 1) as u64 || before.evictions != 0 {
        return Err(
            format!("history fixture did not retain all prepared states: {before:?}").into(),
        );
    }

    let mut timings = Vec::with_capacity(HISTORY_STATES * 2);
    for _ in 0..HISTORY_STATES {
        runtime.session_mut().undo()?;
        let started = Instant::now();
        black_box(runtime.prepared_page(page_id)?.stats().occupied_cells);
        timings.push(started.elapsed());
    }
    for _ in 0..HISTORY_STATES {
        runtime.session_mut().redo()?;
        let started = Instant::now();
        black_box(runtime.prepared_page(page_id)?.stats().occupied_cells);
        timings.push(started.elapsed());
    }

    let stats = runtime.prepared_page_cache_stats();
    if stats.builds != before.builds
        || stats.misses != before.misses
        || stats.hits != before.hits + timings.len() as u64
    {
        return Err(format!(
            "Undo/Redo did not reuse retained prepared snapshots: before={before:?} after={stats:?}"
        )
        .into());
    }

    Ok(HistoryMeasurement {
        samples: timings.len(),
        timings: summarize(timings),
        stats,
    })
}

fn measure_eviction_rebuild(
    session: &EditorSession,
    page_id: PageId,
    element_id: ElementId,
) -> Result<EvictionMeasurement, Box<dyn Error>> {
    let mut runtime = EditorRuntime::with_cache_capacity(session.clone(), CACHE_CAPACITY)?;
    black_box(runtime.prepared_page(page_id)?.stats().occupied_cells);

    for step in 0..EVICTION_EDITS {
        move_element(&mut runtime, element_id, step + 1)?;
        black_box(runtime.prepared_page(page_id)?.stats().occupied_cells);
    }

    let after_edits = runtime.prepared_page_cache_stats();
    if after_edits.entries != CACHE_CAPACITY || after_edits.evictions != 1 {
        return Err(
            format!("eviction fixture did not evict the oldest state: {after_edits:?}").into(),
        );
    }

    // The three newest historical states must still be cache hits. The fourth Undo
    // returns to the initial state, which was evicted when state 4 was prepared.
    for _ in 0..(EVICTION_EDITS - 1) {
        runtime.session_mut().undo()?;
        black_box(runtime.prepared_page(page_id)?.stats().occupied_cells);
    }
    runtime.session_mut().undo()?;
    let started = Instant::now();
    black_box(runtime.prepared_page(page_id)?.stats().occupied_cells);
    let elapsed = started.elapsed();

    let stats = runtime.prepared_page_cache_stats();
    if stats.builds != (EVICTION_EDITS + 2) as u64
        || stats.evictions != 2
        || stats.misses != (EVICTION_EDITS + 2) as u64
    {
        return Err(
            format!("evicted historical state was not rebuilt exactly once: {stats:?}").into(),
        );
    }

    Ok(EvictionMeasurement { elapsed, stats })
}

fn move_element(
    runtime: &mut EditorRuntime,
    element_id: ElementId,
    step: usize,
) -> Result<(), Box<dyn Error>> {
    runtime.session_mut().execute(EditCommand::MoveElements {
        element_ids: vec![element_id],
        delta_mm: Point {
            x: 0.125 + step as f64 * 0.001,
            y: 0.0,
        },
    })?;
    Ok(())
}

fn require_stats(
    actual: PreparedPageCacheStats,
    expected: PreparedPageCacheStats,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    if actual != expected {
        return Err(
            format!("{label} stats mismatch: expected={expected:?} actual={actual:?}").into(),
        );
    }
    Ok(())
}

fn summarize(mut timings: Vec<Duration>) -> TimingSummary {
    timings.sort_unstable();
    TimingSummary {
        p50: timings[percentile_index(timings.len(), 50)],
        p95: timings[percentile_index(timings.len(), 95)],
        p99: timings[percentile_index(timings.len(), 99)],
        max: *timings.last().expect("benchmark timings are non-empty"),
    }
}

fn percentile_index(len: usize, percentile: usize) -> usize {
    ((len - 1) * percentile / 100).min(len - 1)
}

fn synthetic_session(count: usize) -> Result<(EditorSession, PageId, ElementId), Box<dyn Error>> {
    if count == 0 {
        return Err("benchmark element count must be greater than zero".into());
    }

    let page_id = PageId::new();
    let layer_id = LayerId::new();
    let document_id = DocumentId::new();
    let rows = count.div_ceil(COLUMNS);
    let world_size = Size {
        width: COLUMNS as f64 * CELL_WIDTH_MM,
        height: rows as f64 * CELL_HEIGHT_MM,
    };

    let mut elements = Vec::with_capacity(count);
    let mut roots = Vec::with_capacity(count);
    let mut movable_element_id = None;
    for index in 0..count {
        let element_id = ElementId::new();
        if index == 0 {
            movable_element_id = Some(element_id);
        }
        let column = index % COLUMNS;
        let row = index / COLUMNS;
        let x = column as f64 * CELL_WIDTH_MM + 3.0;
        let y = row as f64 * CELL_HEIGHT_MM + 3.0;
        roots.push(element_id);
        elements.push(synthetic_element(element_id, index, x, y));
    }

    let document = Document {
        id: document_id,
        name: format!("Prepared cache benchmark {count}"),
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

    let session = EditorSession::from_artifact(NextArtifact::document(document))?;
    Ok((
        session,
        page_id,
        movable_element_id.expect("positive count creates one movable benchmark element"),
    ))
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
