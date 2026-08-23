import assert from "node:assert/strict";
import test from "node:test";

import {
  SnapContractError,
  buildRulerTicks,
  snapMoveDelta,
  visualBoundsMm,
} from "./snapping.mjs";

const pageSize = { width: 210, height: 297 };

function snapElement(elementId, x, y, width = 10, height = 10, rotationDeg = 0) {
  return { elementId, boundsMm: { x, y, width, height }, rotationDeg };
}

test("grid snapping aligns the closest moving anchor in document millimetres", () => {
  const result = snapMoveDelta({
    deltaMm: { x: 3.8, y: 0 },
    elementIds: ["moving"],
    elements: [snapElement("moving", 1, 1)],
    pageSize,
    gridStepMm: 5,
    thresholdMm: 1.5,
    objectEnabled: false,
  });

  assert.equal(result.deltaMm.x, 4);
  assert.equal(result.guides.x.source, "grid");
  assert.equal(result.guides.x.positionMm, 5);
});

test("object guides take priority over grid guides and can align centers", () => {
  const result = snapMoveDelta({
    deltaMm: { x: 8.6, y: 0 },
    elementIds: ["moving"],
    elements: [snapElement("moving", 0, 0, 10, 10), snapElement("target", 18.8, 0, 10, 10)],
    pageSize,
    gridStepMm: 5,
    thresholdMm: 1,
  });

  assert.ok(Math.abs(result.deltaMm.x - 8.8) < 1e-9);
  assert.equal(result.guides.x.source, "object");
  assert.equal(result.guides.x.targetElementId, "target");
  assert.equal(result.guides.x.positionMm, 18.8);
});

test("moving elements are excluded from their own object snap targets", () => {
  const result = snapMoveDelta({
    deltaMm: { x: 0.4, y: 0.4 },
    elementIds: ["moving"],
    elements: [snapElement("moving", 10, 10)],
    pageSize,
    gridEnabled: false,
    objectEnabled: true,
    thresholdMm: 0.5,
  });

  assert.deepEqual(result.deltaMm, { x: 0.4, y: 0.4 });
  assert.equal(result.guides.x, null);
  assert.equal(result.guides.y, null);
});

test("page edges and center are renderer-neutral object snap guides", () => {
  const result = snapMoveDelta({
    deltaMm: { x: 94.6, y: 0 },
    elementIds: ["moving"],
    elements: [snapElement("moving", 0, 0, 10, 10)],
    pageSize,
    gridEnabled: false,
    thresholdMm: 0.5,
  });

  assert.equal(result.deltaMm.x, 95);
  assert.equal(result.guides.x.source, "page");
  assert.equal(result.guides.x.positionMm, 105);
});

test("rotated visual bounds use a normalized axis-aligned document-space envelope", () => {
  const bounds = visualBoundsMm({ x: 10, y: 20, width: 20, height: 10 }, 90);
  assert.ok(Math.abs(bounds.x - 15) < 1e-9);
  assert.ok(Math.abs(bounds.y - 15) < 1e-9);
  assert.ok(Math.abs(bounds.width - 10) < 1e-9);
  assert.ok(Math.abs(bounds.height - 20) < 1e-9);
});

test("disabled snapping returns the raw delta exactly", () => {
  const result = snapMoveDelta({
    deltaMm: { x: 1.234, y: -5.678 },
    elementIds: ["moving"],
    elements: [snapElement("moving", 0, 0)],
    pageSize,
    gridEnabled: false,
    objectEnabled: false,
  });
  assert.deepEqual(result.deltaMm, { x: 1.234, y: -5.678 });
  assert.deepEqual(result.guides, { x: null, y: null });
});

test("ruler ticks preserve millimetre positions and major labels", () => {
  const ticks = buildRulerTicks(23, { minorStepMm: 5, majorStepMm: 10 });
  assert.deepEqual(
    ticks.map((tick) => [tick.positionMm, tick.major, tick.label]),
    [
      [0, true, "0"],
      [5, false, null],
      [10, true, "10"],
      [15, false, null],
      [20, true, "20"],
      [23, true, "23"],
    ],
  );
});

test("invalid snap geometry fails explicitly instead of producing NaN movement", () => {
  assert.throws(
    () =>
      snapMoveDelta({
        deltaMm: { x: 0, y: 0 },
        elementIds: ["moving"],
        elements: [snapElement("moving", Number.NaN, 0)],
        pageSize,
      }),
    SnapContractError,
  );
});
