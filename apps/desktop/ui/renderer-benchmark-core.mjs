const SVG_NS = 'http://www.w3.org/2000/svg';

export const RENDERER_BENCHMARK_SCHEMA = 'diagramdesigner-next-svg-dom-v2';
export const TARGET_PHYSICAL_PX = Object.freeze({ width: 3840, height: 2160 });
export const SVG_VIEWPORT_UNITS = Object.freeze({ width: 960, height: 540 });
export const BENCHMARK_COUNTS = Object.freeze([5000, 20000]);
export const BENCHMARK_MODES = Object.freeze(['culled', 'full']);
export const BENCHMARK_FRAMES = 120;
export const CULLED_DOM_NODE_LIMIT = 1500;
export const TARGET_FPS = 60;
export const FRAME_BUDGET_MS = 1000 / TARGET_FPS;
export const FRAME_P95_TIMING_TOLERANCE = 0.05;
export const FRAME_P95_LIMIT_MS = FRAME_BUDGET_MS * (1 + FRAME_P95_TIMING_TOLERANCE);
export const UPDATE_P95_LIMIT_MS = FRAME_BUDGET_MS;

const COLUMNS = 100;
const CELL = Object.freeze({ width: 30, height: 22 });
const CULL_MARGIN = 12;
const PERFORMANCE_CRITERIA = Object.freeze({
  targetFps: TARGET_FPS,
  updateP95LimitMs: UPDATE_P95_LIMIT_MS,
  frameP95LimitMs: FRAME_P95_LIMIT_MS,
  frameP95TimingTolerance: FRAME_P95_TIMING_TOLERANCE,
  culledDomNodeLimit: CULLED_DOM_NODE_LIMIT,
});

export function evaluateRendererBenchmark({ environment, results } = {}) {
  const normalizedResults = Array.isArray(results) ? results : [];
  const reasons = [];

  if (!environment || environment.runtime !== 'tauri-webview2' || environment.platform !== 'windows') {
    reasons.push('benchmark must run inside the Windows Tauri/WebView2 runtime');
  }

  const clientWidth = Number(environment?.clientWidthPx);
  const clientHeight = Number(environment?.clientHeightPx);
  if (
    !Number.isFinite(clientWidth) ||
    !Number.isFinite(clientHeight) ||
    clientWidth < TARGET_PHYSICAL_PX.width ||
    clientHeight < TARGET_PHYSICAL_PX.height
  ) {
    reasons.push(
      `physical WebView client area must be at least ${TARGET_PHYSICAL_PX.width}×${TARGET_PHYSICAL_PX.height}`,
    );
  }

  if (reasons.length > 0) {
    return freezeVerdict('environment_not_eligible', false, reasons);
  }

  for (const count of BENCHMARK_COUNTS) {
    const result = normalizedResults.find(
      (candidate) => candidate?.nodes_requested === count && candidate?.mode === 'culled',
    );
    if (!result) {
      reasons.push(`missing culled ${count.toLocaleString('en-US')} result`);
      continue;
    }

    const stageWidth = Number(result.stage_physical_px?.width);
    const stageHeight = Number(result.stage_physical_px?.height);
    if (!Number.isFinite(stageWidth) || !Number.isFinite(stageHeight)) {
      reasons.push(`culled ${count.toLocaleString('en-US')} has no finite stage physical area`);
    } else if (
      stageWidth < TARGET_PHYSICAL_PX.width ||
      stageHeight < TARGET_PHYSICAL_PX.height
    ) {
      reasons.push(
        `culled ${count.toLocaleString('en-US')} stage physical area ${stageWidth}×${stageHeight} is below ${TARGET_PHYSICAL_PX.width}×${TARGET_PHYSICAL_PX.height}`,
      );
    }

    const updateP95 = Number(result.update_ms_p95);
    if (!Number.isFinite(updateP95)) {
      reasons.push(`culled ${count.toLocaleString('en-US')} has no finite update p95`);
    } else if (updateP95 > UPDATE_P95_LIMIT_MS) {
      reasons.push(
        `culled ${count.toLocaleString('en-US')} update p95 ${updateP95.toFixed(3)} ms exceeds 60 fps work budget ${UPDATE_P95_LIMIT_MS.toFixed(3)} ms`,
      );
    }

    const frameP95 = Number(result.frame_ms_p95);
    if (!Number.isFinite(frameP95)) {
      reasons.push(`culled ${count.toLocaleString('en-US')} has no finite frame p95`);
    } else if (frameP95 > FRAME_P95_LIMIT_MS) {
      reasons.push(
        `culled ${count.toLocaleString('en-US')} frame p95 ${frameP95.toFixed(3)} ms exceeds 60 fps timing limit ${FRAME_P95_LIMIT_MS.toFixed(3)} ms (5% rAF/VSync tolerance)`,
      );
    }

    if (result.long_tasks_observed === null || result.long_tasks_observed === undefined) {
      reasons.push(`culled ${count.toLocaleString('en-US')} Long Task evidence is unavailable`);
    } else if (Number(result.long_tasks_observed) > 0) {
      reasons.push(
        `culled ${count.toLocaleString('en-US')} observed ${Number(result.long_tasks_observed)} Long Tasks`,
      );
    }

    const domMax = Number(result.dom_nodes_max);
    if (!Number.isFinite(domMax)) {
      reasons.push(`culled ${count.toLocaleString('en-US')} has no finite DOM maximum`);
    } else if (domMax > CULLED_DOM_NODE_LIMIT) {
      reasons.push(
        `culled ${count.toLocaleString('en-US')} DOM maximum ${domMax} exceeds bounded limit ${CULLED_DOM_NODE_LIMIT}`,
      );
    }
  }

  if (
    reasons.some(
      (reason) =>
        reason.startsWith('missing ') ||
        reason.includes('unavailable') ||
        reason.includes('no finite') ||
        reason.includes('stage physical area'),
    )
  ) {
    return freezeVerdict('measurement_incomplete', false, reasons);
  }
  if (reasons.length > 0) {
    return freezeVerdict('fallback_required', false, reasons);
  }

  return freezeVerdict('performance_gate_pass', true, [
    'culled 5k and 20k keep update p95 inside the 60 fps work budget and frame p95 inside the bounded rAF/VSync timing tolerance',
    'final renderer acceptance still requires the corresponding correctness/fidelity evidence and representative-hardware review',
  ]);
}

export async function runSvgDomCase({
  count,
  mode,
  svg,
  scene,
  windowRef = globalThis.window,
  documentRef = globalThis.document,
} = {}) {
  validateCase(count, mode, svg, scene, windowRef, documentRef);
  const descriptors = buildDescriptors(count);
  const world = worldSize(count);
  const updateMs = [];
  const frameMs = [];
  const domCounts = [];
  let previousRaf = null;
  let longTasks = 0;
  let observer = null;

  if ('PerformanceObserver' in windowRef) {
    try {
      observer = new windowRef.PerformanceObserver((list) => {
        longTasks += list.getEntries().length;
      });
      observer.observe({ entryTypes: ['longtask'] });
    } catch {
      observer = null;
    }
  }

  scene.replaceChildren();
  if (mode === 'full') replaceScene(descriptors, scene, documentRef);
  await nextFrame(windowRef);
  await nextFrame(windowRef);
  const stagePhysicalPx = measurePhysicalSurface(svg, windowRef);

  try {
    for (let frame = 0; frame < BENCHMARK_FRAMES; frame += 1) {
      const rafTime = await nextFrame(windowRef);
      if (previousRaf !== null) frameMs.push(rafTime - previousRaf);
      previousRaf = rafTime;

      const viewport = viewportAt(frame, world);
      const started = windowRef.performance.now();
      if (mode === 'culled') {
        replaceScene(
          descriptors.filter((item) => intersects(item, viewport)),
          scene,
          documentRef,
        );
      }
      svg.setAttribute(
        'viewBox',
        `${viewport.x} ${viewport.y} ${viewport.width} ${viewport.height}`,
      );
      void scene.getBoundingClientRect();
      updateMs.push(windowRef.performance.now() - started);
      domCounts.push(scene.childElementCount);
    }

    await nextFrame(windowRef);
  } finally {
    observer?.disconnect();
  }

  return Object.freeze({
    benchmark: RENDERER_BENCHMARK_SCHEMA,
    nodes_requested: count,
    mode,
    frames: BENCHMARK_FRAMES,
    target_screen_px: TARGET_PHYSICAL_PX,
    svg_viewport_units: SVG_VIEWPORT_UNITS,
    browser: windowRef.navigator?.userAgent ?? null,
    device_pixel_ratio: windowRef.devicePixelRatio ?? null,
    hardware_concurrency: windowRef.navigator?.hardwareConcurrency ?? null,
    gpu: readWebGlRenderer(documentRef),
    css_client_px: Object.freeze({
      width: windowRef.innerWidth ?? null,
      height: windowRef.innerHeight ?? null,
    }),
    stage_physical_px: stagePhysicalPx,
    dom_nodes_min: Math.min(...domCounts),
    dom_nodes_max: Math.max(...domCounts),
    update_ms_p50: percentile(updateMs, 0.5),
    update_ms_p95: percentile(updateMs, 0.95),
    update_ms_p99: percentile(updateMs, 0.99),
    update_ms_max: Math.max(...updateMs),
    frame_ms_p50: percentile(frameMs, 0.5),
    frame_ms_p95: percentile(frameMs, 0.95),
    frame_ms_p99: percentile(frameMs, 0.99),
    frame_ms_max: Math.max(...frameMs),
    frame_budget_ms: FRAME_BUDGET_MS,
    frame_p95_limit_ms: FRAME_P95_LIMIT_MS,
    frames_over_16_67_ms: frameMs.filter((value) => value > FRAME_BUDGET_MS).length,
    frames_over_p95_limit_ms: frameMs.filter((value) => value > FRAME_P95_LIMIT_MS).length,
    long_tasks_observed: observer ? longTasks : null,
    generated_at: new Date().toISOString(),
  });
}

export function benchmarkCaseMatrix() {
  return BENCHMARK_COUNTS.flatMap((count) => BENCHMARK_MODES.map((mode) => Object.freeze({ count, mode })));
}

function freezeVerdict(status, performancePass, reasons) {
  return Object.freeze({
    status,
    performancePass,
    criteria: PERFORMANCE_CRITERIA,
    reasons: Object.freeze([...reasons]),
  });
}

function validateCase(count, mode, svg, scene, windowRef, documentRef) {
  if (!BENCHMARK_COUNTS.includes(count)) {
    throw new TypeError(`unsupported benchmark node count: ${count}`);
  }
  if (!BENCHMARK_MODES.includes(mode)) {
    throw new TypeError(`unsupported benchmark mode: ${mode}`);
  }
  if (!svg || typeof svg.setAttribute !== 'function' || !scene || typeof scene.replaceChildren !== 'function') {
    throw new TypeError('benchmark requires SVG and scene DOM elements');
  }
  if (!windowRef?.performance || typeof windowRef.requestAnimationFrame !== 'function') {
    throw new TypeError('benchmark requires a browser animation/performance environment');
  }
  if (!documentRef || typeof documentRef.createElementNS !== 'function') {
    throw new TypeError('benchmark requires a DOM document');
  }
}

function descriptor(index) {
  const column = index % COLUMNS;
  const row = Math.floor(index / COLUMNS);
  return {
    id: index,
    kind: index % 10,
    x: column * CELL.width + 3,
    y: row * CELL.height + 3,
    width: 20,
    height: 14,
    rotation: index % 17 === 0 ? 15 : 0,
  };
}

function buildDescriptors(count) {
  return Array.from({ length: count }, (_, index) => descriptor(index));
}

function worldSize(count) {
  return {
    width: COLUMNS * CELL.width,
    height: Math.ceil(count / COLUMNS) * CELL.height,
  };
}

function viewportAt(frame, world) {
  const maxX = Math.max(0, world.width - SVG_VIEWPORT_UNITS.width);
  const maxY = Math.max(0, world.height - SVG_VIEWPORT_UNITS.height);
  return {
    x: maxX * (((frame * 37) % 101) / 100),
    y: maxY * (((frame * 53) % 101) / 100),
    width: SVG_VIEWPORT_UNITS.width,
    height: SVG_VIEWPORT_UNITS.height,
  };
}

function intersects(item, viewport) {
  return !(
    item.x + item.width < viewport.x - CULL_MARGIN ||
    item.x > viewport.x + viewport.width + CULL_MARGIN ||
    item.y + item.height < viewport.y - CULL_MARGIN ||
    item.y > viewport.y + viewport.height + CULL_MARGIN
  );
}

function createNode(item, documentRef) {
  let node;
  switch (item.kind) {
    case 0:
    case 1:
    case 2:
    case 3:
      node = documentRef.createElementNS(SVG_NS, 'rect');
      node.setAttribute('x', item.x);
      node.setAttribute('y', item.y);
      node.setAttribute('width', item.width);
      node.setAttribute('height', item.height);
      node.setAttribute('rx', item.kind % 2 === 0 ? '0' : '1.5');
      break;
    case 4:
    case 5:
      node = documentRef.createElementNS(SVG_NS, 'ellipse');
      node.setAttribute('cx', item.x + item.width / 2);
      node.setAttribute('cy', item.y + item.height / 2);
      node.setAttribute('rx', item.width / 2);
      node.setAttribute('ry', item.height / 2);
      break;
    case 6:
      node = documentRef.createElementNS(SVG_NS, 'text');
      node.setAttribute('x', item.x + 1);
      node.setAttribute('y', item.y + 9);
      node.textContent = `N${item.id}`;
      break;
    case 7:
      node = documentRef.createElementNS(SVG_NS, 'line');
      node.setAttribute('x1', item.x);
      node.setAttribute('y1', item.y + item.height / 2);
      node.setAttribute('x2', item.x + item.width);
      node.setAttribute('y2', item.y + item.height / 2);
      break;
    case 8:
      node = documentRef.createElementNS(SVG_NS, 'polygon');
      node.setAttribute(
        'points',
        `${item.x},${item.y + item.height} ${item.x + item.width / 2},${item.y} ${item.x + item.width},${item.y + item.height}`,
      );
      break;
    default:
      node = documentRef.createElementNS(SVG_NS, 'path');
      node.setAttribute(
        'd',
        `M ${item.x} ${item.y} h ${item.width} v ${item.height} h -${item.width} z`,
      );
      break;
  }

  node.setAttribute('data-id', item.id);
  node.setAttribute('fill', 'none');
  node.setAttribute('stroke', 'currentColor');
  node.setAttribute('stroke-width', '0.35');
  if (item.kind === 6) {
    node.setAttribute('fill', 'currentColor');
    node.setAttribute('stroke', 'none');
    node.setAttribute('font-size', '5');
  }
  if (item.rotation !== 0) {
    const cx = item.x + item.width / 2;
    const cy = item.y + item.height / 2;
    node.setAttribute('transform', `rotate(${item.rotation} ${cx} ${cy})`);
  }
  return node;
}

function replaceScene(items, scene, documentRef) {
  const fragment = documentRef.createDocumentFragment();
  for (const item of items) fragment.appendChild(createNode(item, documentRef));
  scene.replaceChildren(fragment);
}

function percentile(values, p) {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * p));
  return sorted[index];
}

function nextFrame(windowRef) {
  return new Promise((resolve) => windowRef.requestAnimationFrame(resolve));
}

function measurePhysicalSurface(svg, windowRef) {
  const rect = svg.getBoundingClientRect();
  const ratio = Number(windowRef.devicePixelRatio ?? 1);
  return Object.freeze({
    width: Math.round(rect.width * ratio),
    height: Math.round(rect.height * ratio),
  });
}

function readWebGlRenderer(documentRef) {
  try {
    const canvas = documentRef.createElement('canvas');
    const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
    if (!gl) return null;
    const extension = gl.getExtension('WEBGL_debug_renderer_info');
    if (!extension) return null;
    return Object.freeze({
      vendor: gl.getParameter(extension.UNMASKED_VENDOR_WEBGL),
      renderer: gl.getParameter(extension.UNMASKED_RENDERER_WEBGL),
    });
  } catch {
    return null;
  }
}
