import {
  benchmarkCaseMatrix,
  evaluateRendererBenchmark,
  runSvgDomCase,
} from './renderer-benchmark-core.mjs';

const invoke = window.__TAURI__?.core?.invoke;

const elements = {
  eligibility: document.querySelector('#eligibility'),
  environment: document.querySelector('#environment'),
  machineModel: document.querySelector('#machine-model'),
  hardwareNotes: document.querySelector('#hardware-notes'),
  runSuite: document.querySelector('#run-suite'),
  copyResult: document.querySelector('#copy-result'),
  close: document.querySelector('#close-benchmark'),
  svg: document.querySelector('#diagram'),
  scene: document.querySelector('#scene'),
  progress: document.querySelector('#progress'),
  result: document.querySelector('#result'),
};

let running = false;
let lastReport = null;

async function readEnvironment() {
  if (!invoke) {
    return {
      runtime: 'browser',
      platform: 'unknown',
      clientWidthPx: Math.round(window.innerWidth * window.devicePixelRatio),
      clientHeightPx: Math.round(window.innerHeight * window.devicePixelRatio),
      scaleFactor: window.devicePixelRatio,
      fullscreen: false,
      monitorWidthPx: null,
      monitorHeightPx: null,
      monitorName: null,
      appVersion: null,
      sourceCommit: null,
      sourceDirty: null,
    };
  }
  return invoke('renderer_benchmark_environment');
}

function renderEnvironment(environment) {
  elements.environment.replaceChildren();
  const rows = [
    ['Runtime', environment.runtime],
    ['Platform', environment.platform],
    ['Client area', `${environment.clientWidthPx} × ${environment.clientHeightPx} px`],
    ['Scale factor', String(environment.scaleFactor)],
    ['Fullscreen', environment.fullscreen ? 'yes' : 'no'],
    [
      'Monitor',
      environment.monitorWidthPx && environment.monitorHeightPx
        ? `${environment.monitorWidthPx} × ${environment.monitorHeightPx} px`
        : 'unknown',
    ],
    ['Monitor name', environment.monitorName ?? 'unknown'],
    ['App version', environment.appVersion ?? 'unknown'],
    ['Source commit', environment.sourceCommit ?? 'unknown'],
    [
      'Source dirty',
      environment.sourceDirty === true
        ? 'yes'
        : environment.sourceDirty === false
          ? 'no'
          : 'unknown',
    ],
  ];

  const fragment = document.createDocumentFragment();
  for (const [label, value] of rows) {
    const row = document.createElement('div');
    const dt = document.createElement('dt');
    const dd = document.createElement('dd');
    dt.textContent = label;
    dd.textContent = value;
    row.append(dt, dd);
    fragment.append(row);
  }
  elements.environment.replaceChildren(fragment);

  const verdict = evaluateRendererBenchmark({ environment, results: [] });
  if (verdict.status === 'environment_not_eligible') {
    elements.eligibility.dataset.state = 'ineligible';
    elements.eligibility.textContent = `Not eligible for ADR-019: ${verdict.reasons.join('; ')}`;
  } else {
    // With an eligible environment and no measurements the evaluator reports an
    // incomplete measurement. This is the desired pre-run state.
    elements.eligibility.dataset.state = 'incomplete';
    elements.eligibility.textContent = 'Native environment eligible; measurement not run yet.';
  }
}

async function runSuite() {
  if (running) return;
  running = true;
  elements.runSuite.disabled = true;
  elements.copyResult.disabled = true;
  elements.result.value = '';
  elements.progress.textContent = 'Reading native environment…';

  try {
    const environment = await readEnvironment();
    renderEnvironment(environment);
    const results = [];

    // Hide every control/result panel and let the SVG stage own the complete
    // fullscreen client area. Two animation frames make the layout transition
    // settle before runSvgDomCase records its physical render-surface dimensions.
    document.body.dataset.benchmarkRunning = 'true';
    await nextAnimationFrame();
    await nextAnimationFrame();

    for (const { count, mode } of benchmarkCaseMatrix()) {
      const result = await runSvgDomCase({
        count,
        mode,
        svg: elements.svg,
        scene: elements.scene,
      });
      results.push(result);
    }

    const verdict = evaluateRendererBenchmark({ environment, results });
    lastReport = Object.freeze({
      report: 'diagramdesigner-next-adr-019-native-v1',
      environment,
      hardware: Object.freeze({
        machineModel: elements.machineModel.value.trim() || null,
        notes: elements.hardwareNotes.value.trim() || null,
      }),
      measurements: Object.freeze(results),
      performanceVerdict: verdict,
      finalRendererDecision: 'not-made-by-benchmark',
      generatedAt: new Date().toISOString(),
    });
    elements.result.value = JSON.stringify(lastReport, null, 2);
    elements.copyResult.disabled = false;

    if (verdict.status === 'performance_gate_pass') {
      elements.eligibility.dataset.state = 'eligible';
      elements.eligibility.textContent =
        'Measured performance gate passed. Final renderer acceptance still requires correctness/fidelity evidence and representative-hardware review.';
    } else if (verdict.status === 'fallback_required') {
      elements.eligibility.dataset.state = 'ineligible';
      elements.eligibility.textContent = `SVG performance gate failed: ${verdict.reasons.join('; ')}`;
    } else {
      elements.eligibility.dataset.state = 'incomplete';
      elements.eligibility.textContent = `${verdict.status}: ${verdict.reasons.join('; ')}`;
    }

    let evidenceMessage = '';
    if (invoke) {
      elements.progress.textContent = 'Suite complete — saving native evidence…';
      try {
        const evidence = await invoke('persist_renderer_benchmark_evidence', {
          request: { report: lastReport },
        });
        evidenceMessage = ` — evidence saved: ${evidence.path}`;
        if (evidence.cleanupWarning) {
          evidenceMessage += ` (cleanup warning: ${evidence.cleanupWarning})`;
        }
      } catch (error) {
        evidenceMessage = ` — evidence save failed: ${String(error?.message ?? error)}`;
      }
    }
    elements.progress.textContent = `Suite complete${evidenceMessage}`;
  } catch (error) {
    lastReport = null;
    elements.result.value = String(error?.stack ?? error);
    elements.eligibility.dataset.state = 'ineligible';
    elements.eligibility.textContent = 'Benchmark failed';
    elements.progress.textContent = 'Failed';
  } finally {
    delete document.body.dataset.benchmarkRunning;
    running = false;
    elements.runSuite.disabled = false;
  }
}

function nextAnimationFrame() {
  return new Promise((resolve) => window.requestAnimationFrame(resolve));
}

async function copyResult() {
  if (!lastReport) return;
  try {
    await navigator.clipboard.writeText(elements.result.value);
    elements.progress.textContent = 'JSON copied';
  } catch {
    elements.result.focus();
    elements.result.select();
    elements.progress.textContent = 'Clipboard unavailable — result selected for manual copy';
  }
}

async function closeBenchmark() {
  if (!invoke) {
    window.close();
    return;
  }
  try {
    await invoke('close_renderer_benchmark');
  } catch (error) {
    elements.progress.textContent = String(error?.message ?? error);
  }
}

elements.runSuite.addEventListener('click', () => void runSuite());
elements.copyResult.addEventListener('click', () => void copyResult());
elements.close.addEventListener('click', () => void closeBenchmark());
window.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && !running) {
    event.preventDefault();
    void closeBenchmark();
  }
});

void readEnvironment()
  .then(renderEnvironment)
  .catch((error) => {
    elements.eligibility.dataset.state = 'ineligible';
    elements.eligibility.textContent = String(error?.message ?? error);
  });
