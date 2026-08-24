import test from 'node:test';
import assert from 'node:assert/strict';

import {
  ConnectorGestureController,
  buildOrthogonalPreviewPoints,
  normalizeConnectorKind,
} from './connector-gesture.mjs';

test('connector gesture keeps pointer updates transient and emits one semantic create intent', () => {
  const controller = new ConnectorGestureController({
    screenToDocument: ({ xPx, yPx }) => ({ x: xPx / 2, y: yPx / 4 }),
    minimumLengthMm: 0.5,
  });

  const begin = controller.begin({
    pointerId: 7,
    screenPoint: { xPx: 20, yPx: 40 },
    connectorKind: 'straight',
  });
  assert.deepEqual(begin, {
    kind: 'connector-preview',
    pointerId: 7,
    connectorKind: 'straight',
    startMm: { x: 10, y: 10 },
    endMm: { x: 10, y: 10 },
  });

  const update = controller.update({
    pointerId: 7,
    screenPoint: { xPx: 50, yPx: 80 },
  });
  assert.deepEqual(update.endMm, { x: 25, y: 20 });
  assert.equal(controller.isActive, true);

  const commit = controller.finish({
    pointerId: 7,
    screenPoint: { xPx: 60, yPx: 100 },
  });
  assert.deepEqual(commit, {
    kind: 'create-connector',
    connectorKind: 'straight',
    startMm: { x: 10, y: 10 },
    endMm: { x: 30, y: 25 },
  });
  assert.equal(controller.isActive, false);
  assert.equal(controller.finish({ pointerId: 7, screenPoint: { xPx: 60, yPx: 100 } }), null);
});

test('connector gesture ignores clicks shorter than the semantic minimum', () => {
  const controller = new ConnectorGestureController({
    screenToDocument: ({ xPx, yPx }) => ({ x: xPx, y: yPx }),
    minimumLengthMm: 1,
  });
  controller.begin({
    pointerId: 1,
    screenPoint: { xPx: 5, yPx: 5 },
    connectorKind: 'orthogonal',
  });
  assert.equal(
    controller.finish({ pointerId: 1, screenPoint: { xPx: 5.4, yPx: 5.4 } }),
    null,
  );
  assert.equal(controller.isActive, false);
});

test('cancel clears a gesture without creating semantic state', () => {
  const controller = new ConnectorGestureController({
    screenToDocument: ({ xPx, yPx }) => ({ x: xPx, y: yPx }),
  });
  controller.begin({
    pointerId: 3,
    screenPoint: { xPx: 1, yPx: 2 },
    connectorKind: 'straight',
  });
  assert.equal(controller.cancel(3), true);
  assert.equal(controller.cancel(3), false);
  assert.equal(controller.isActive, false);
});

test('orthogonal preview follows the renderer free-endpoint routing axis', () => {
  assert.deepEqual(buildOrthogonalPreviewPoints({ x: 10, y: 10 }, { x: 50, y: 30 }), [
    { x: 10, y: 10 },
    { x: 30, y: 10 },
    { x: 30, y: 20 },
    { x: 30, y: 30 },
    { x: 50, y: 30 },
  ]);

  assert.deepEqual(buildOrthogonalPreviewPoints({ x: 20, y: 10 }, { x: 30, y: 70 }), [
    { x: 20, y: 10 },
    { x: 20, y: 40 },
    { x: 25, y: 40 },
    { x: 30, y: 40 },
    { x: 30, y: 70 },
  ]);
});

test('connector kind validation rejects unsupported frontend tool state', () => {
  assert.equal(normalizeConnectorKind('straight'), 'straight');
  assert.equal(normalizeConnectorKind('orthogonal'), 'orthogonal');
  assert.throws(() => normalizeConnectorKind('curve'), /unsupported connector kind/);
});
