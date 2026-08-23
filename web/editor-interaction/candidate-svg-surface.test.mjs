import assert from "node:assert/strict";
import test from "node:test";

import { mapClientPointToViewBox } from "../../apps/desktop/ui/candidate-svg-surface.mjs";

test("candidate SVG point mapping preserves proportional document coordinates", () => {
  const point = mapClientPointToViewBox(
    { left: 100, top: 50, width: 400, height: 600 },
    { x: 0, y: 0, width: 200, height: 300 },
    { xPx: 300, yPx: 350 },
  );

  assert.deepEqual(point, { x: 100, y: 150 });
  assert.equal(Object.isFrozen(point), true);
});

test("candidate SVG point mapping honors non-zero viewBox and screen origins", () => {
  assert.deepEqual(
    mapClientPointToViewBox(
      { left: 20, top: 40, width: 200, height: 100 },
      { x: 10, y: 30, width: 80, height: 40 },
      { xPx: 70, yPx: 65 },
    ),
    { x: 30, y: 40 },
  );
});

test("candidate SVG point mapping rejects invalid screen geometry", () => {
  assert.throws(
    () =>
      mapClientPointToViewBox(
        { left: 0, top: 0, width: 0, height: 100 },
        { x: 0, y: 0, width: 10, height: 10 },
        { xPx: 1, yPx: 1 },
      ),
    /finite geometry/,
  );
});

test("candidate SVG point mapping rejects invalid viewBox size", () => {
  assert.throws(
    () =>
      mapClientPointToViewBox(
        { left: 0, top: 0, width: 100, height: 100 },
        { x: 0, y: 0, width: 10, height: -1 },
        { xPx: 1, yPx: 1 },
      ),
    /greater than zero/,
  );
});
