import assert from 'node:assert/strict';
import test from 'node:test';

import {
  CULLED_DOM_NODE_LIMIT,
  TARGET_PHYSICAL_PX,
  evaluateRendererBenchmark,
} from '../../apps/desktop/ui/renderer-benchmark-core.mjs';

const eligibleEnvironment = Object.freeze({
  runtime: 'tauri-webview2',
  platform: 'windows',
  clientWidthPx: 3840,
  clientHeightPx: 2160,
});

function passingResult(nodes) {
  return {
    nodes_requested: nodes,
    mode: 'culled',
    stage_physical_px: { ...TARGET_PHYSICAL_PX },
    frame_ms_p95: 15.5,
    long_tasks_observed: 0,
    dom_nodes_max: CULLED_DOM_NODE_LIMIT - 1,
  };
}

test('native performance gate refuses a non-4K physical WebView client area', () => {
  const verdict = evaluateRendererBenchmark({
    environment: { ...eligibleEnvironment, clientHeightPx: 2100 },
    results: [passingResult(5000), passingResult(20000)],
  });
  assert.equal(verdict.status, 'environment_not_eligible');
  assert.equal(verdict.performancePass, false);
});

test('culled 5k and 20k passing measurements satisfy the performance gate', () => {
  const verdict = evaluateRendererBenchmark({
    environment: eligibleEnvironment,
    results: [passingResult(5000), passingResult(20000)],
  });
  assert.equal(verdict.status, 'performance_gate_pass');
  assert.equal(verdict.performancePass, true);
});

test('eligible window does not count when the measured SVG stage is smaller than physical 4K', () => {
  const verdict = evaluateRendererBenchmark({
    environment: eligibleEnvironment,
    results: [
      passingResult(5000),
      {
        ...passingResult(20000),
        stage_physical_px: { width: 3500, height: 1900 },
      },
    ],
  });
  assert.equal(verdict.status, 'measurement_incomplete');
  assert.equal(verdict.performancePass, false);
  assert.match(verdict.reasons.join(' '), /stage physical area/);
});

test('a material 20k p95 miss requires the renderer fallback path', () => {
  const verdict = evaluateRendererBenchmark({
    environment: eligibleEnvironment,
    results: [passingResult(5000), { ...passingResult(20000), frame_ms_p95: 19.2 }],
  });
  assert.equal(verdict.status, 'fallback_required');
  assert.equal(verdict.performancePass, false);
});

test('recurring Long Tasks or an unbounded culled DOM fail the measured gate', () => {
  const verdict = evaluateRendererBenchmark({
    environment: eligibleEnvironment,
    results: [
      { ...passingResult(5000), long_tasks_observed: 2 },
      { ...passingResult(20000), dom_nodes_max: CULLED_DOM_NODE_LIMIT + 1 },
    ],
  });
  assert.equal(verdict.status, 'fallback_required');
  assert.equal(verdict.performancePass, false);
  assert.equal(verdict.reasons.length, 2);
});

test('full-DOM diagnostic results never substitute for missing culled acceptance evidence', () => {
  const verdict = evaluateRendererBenchmark({
    environment: eligibleEnvironment,
    results: [
      { ...passingResult(5000), mode: 'full' },
      { ...passingResult(20000), mode: 'full' },
    ],
  });
  assert.equal(verdict.status, 'measurement_incomplete');
  assert.equal(verdict.performancePass, false);
});

test('non-Windows or non-WebView2 runtime is never eligible for ADR-019', () => {
  for (const environment of [
    { ...eligibleEnvironment, platform: 'linux' },
    { ...eligibleEnvironment, runtime: 'browser' },
  ]) {
    const verdict = evaluateRendererBenchmark({
      environment,
      results: [passingResult(5000), passingResult(20000)],
    });
    assert.equal(verdict.status, 'environment_not_eligible');
  }
});
