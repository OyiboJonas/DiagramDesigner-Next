import test from 'node:test';
import assert from 'node:assert/strict';

import {
  TransformGestureController,
  resizeRotatedBounds,
} from './transform-gesture.mjs';

const original = { x: 10, y: 10, width: 20, height: 20 };

for (const [handle, pointerMm, expected] of [
  ['nw', { x: 5, y: 5 }, { x: 5, y: 5, width: 25, height: 25 }],
  ['n', { x: 20, y: 5 }, { x: 10, y: 5, width: 20, height: 25 }],
  ['ne', { x: 35, y: 5 }, { x: 10, y: 5, width: 25, height: 25 }],
  ['e', { x: 35, y: 20 }, { x: 10, y: 10, width: 25, height: 20 }],
  ['se', { x: 35, y: 35 }, { x: 10, y: 10, width: 25, height: 25 }],
  ['s', { x: 20, y: 35 }, { x: 10, y: 10, width: 20, height: 25 }],
  ['sw', { x: 5, y: 35 }, { x: 5, y: 10, width: 25, height: 25 }],
  ['w', { x: 5, y: 20 }, { x: 5, y: 10, width: 25, height: 20 }],
]) {
  test(`resize handle ${handle} preserves its opposite side`, () => {
    assert.deepEqual(
      resizeRotatedBounds({
        boundsMm: original,
        rotationDeg: 0,
        handle,
        pointerMm,
        minimumSizeMm: 1,
        pageSize: { width: 100, height: 100 },
      }),
      expected,
    );
  });
}

test('rotated resize follows local object axes and keeps the opposite visual edge fixed', () => {
  const resized = resizeRotatedBounds({
    boundsMm: { x: 10, y: 20, width: 40, height: 20 },
    rotationDeg: 90,
    handle: 'e',
    pointerMm: { x: 30, y: 70 },
    minimumSizeMm: 1,
    pageSize: { width: 200, height: 200 },
  });
  assert.ok(Math.abs(resized.x - 0) < 1e-9);
  assert.ok(Math.abs(resized.y - 30) < 1e-9);
  assert.ok(Math.abs(resized.width - 60) < 1e-9);
  assert.ok(Math.abs(resized.height - 20) < 1e-9);
});

test('resize enforces a positive semantic minimum size', () => {
  assert.deepEqual(
    resizeRotatedBounds({
      boundsMm: original,
      rotationDeg: 0,
      handle: 'e',
      pointerMm: { x: 10.2, y: 20 },
      minimumSizeMm: 1,
      pageSize: { width: 100, height: 100 },
    }),
    { x: 10, y: 10, width: 1, height: 20 },
  );
});

test('rotation gesture emits one semantic transform and Shift snaps to 15 degrees', () => {
  const controller = new TransformGestureController({
    screenToDocument: ({ xPx, yPx }) => ({ x: xPx, y: yPx }),
    rotationSnapDeg: 15,
  });
  controller.begin({
    pointerId: 3,
    screenPoint: { xPx: 60, yPx: 50 },
    handle: 'rotate',
    selection: {
      elementId: 'shape-a',
      boundsMm: { x: 40, y: 40, width: 20, height: 20 },
      rotationDeg: 0,
      pageSize: { width: 100, height: 100 },
    },
  });
  const radians = (22 * Math.PI) / 180;
  const commit = controller.finish({
    pointerId: 3,
    screenPoint: {
      xPx: 50 + Math.cos(radians) * 10,
      yPx: 50 + Math.sin(radians) * 10,
    },
    shiftKey: true,
  });
  assert.deepEqual(commit, {
    kind: 'transform-element',
    elementId: 'shape-a',
    boundsMm: { x: 40, y: 40, width: 20, height: 20 },
    rotationDeg: 15,
  });
});

test('cancel drops transient transform state without a commit', () => {
  const controller = new TransformGestureController({
    screenToDocument: ({ xPx, yPx }) => ({ x: xPx, y: yPx }),
  });
  controller.begin({
    pointerId: 7,
    screenPoint: { xPx: 30, yPx: 20 },
    handle: 'e',
    selection: {
      elementId: 'shape-a',
      boundsMm: original,
      rotationDeg: 0,
      pageSize: { width: 100, height: 100 },
    },
  });
  assert.equal(controller.cancel(7), true);
  assert.equal(controller.isActive, false);
  assert.equal(
    controller.finish({ pointerId: 7, screenPoint: { xPx: 40, yPx: 20 } }),
    null,
  );
});
