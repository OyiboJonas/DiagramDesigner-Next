import assert from "node:assert/strict";
import test from "node:test";

import {
  InteractionContractError,
  MoveGestureController,
  bindMovePointerSurface,
} from "./move-gesture.mjs";

function controller(options = {}) {
  return new MoveGestureController({
    screenToDocument: ({ xPx, yPx }) => ({ x: xPx / 10, y: yPx / 10 }),
    ...options,
  });
}

class FakeSurface {
  listeners = new Map();
  captured = new Set();

  addEventListener(type, listener) {
    const listeners = this.listeners.get(type) ?? new Set();
    listeners.add(listener);
    this.listeners.set(type, listeners);
  }

  removeEventListener(type, listener) {
    this.listeners.get(type)?.delete(listener);
  }

  setPointerCapture(pointerId) {
    this.captured.add(pointerId);
  }

  releasePointerCapture(pointerId) {
    this.captured.delete(pointerId);
  }

  hasPointerCapture(pointerId) {
    return this.captured.has(pointerId);
  }

  dispatch(type, values = {}) {
    let prevented = false;
    const event = {
      pointerId: 1,
      button: 0,
      isPrimary: true,
      clientX: 0,
      clientY: 0,
      preventDefault() {
        prevented = true;
      },
      ...values,
    };
    for (const listener of this.listeners.get(type) ?? []) {
      listener(event);
    }
    return { event, prevented };
  }
}

test("pointer updates stay transient until one final semantic commit", () => {
  const drag = controller();
  const initial = drag.begin({
    pointerId: 7,
    screenPoint: { xPx: 20, yPx: 30 },
    elementIds: ["a", "b", "a"],
  });

  assert.deepEqual(initial, {
    kind: "move-preview",
    pointerId: 7,
    elementIds: ["a", "b"],
    deltaMm: { x: 0, y: 0 },
  });
  assert.deepEqual(
    drag.update({ pointerId: 7, screenPoint: { xPx: 50, yPx: 10 } }),
    {
      kind: "move-preview",
      pointerId: 7,
      elementIds: ["a", "b"],
      deltaMm: { x: 3, y: -2 },
    },
  );
  assert.equal(drag.isActive, true);

  const commit = drag.finish({
    pointerId: 7,
    screenPoint: { xPx: 60, yPx: 20 },
  });
  assert.deepEqual(commit, {
    kind: "move-elements",
    elementIds: ["a", "b"],
    deltaMm: { x: 4, y: -1 },
  });
  assert.equal(drag.isActive, false);
});

test("non-owning pointer events are ignored and cancel never commits", () => {
  const drag = controller();
  drag.begin({
    pointerId: 3,
    screenPoint: { xPx: 0, yPx: 0 },
    elementIds: ["a"],
  });

  assert.equal(
    drag.update({ pointerId: 4, screenPoint: { xPx: 100, yPx: 100 } }),
    null,
  );
  assert.equal(drag.cancel(4), false);
  assert.equal(drag.cancel(3), true);
  assert.equal(drag.isActive, false);
  assert.equal(
    drag.finish({ pointerId: 3, screenPoint: { xPx: 100, yPx: 100 } }),
    null,
  );
});

test("zero-distance drag creates no persistent move intent", () => {
  const drag = controller();
  drag.begin({
    pointerId: 1,
    screenPoint: { xPx: 10, yPx: 20 },
    elementIds: ["a"],
  });
  assert.equal(
    drag.finish({ pointerId: 1, screenPoint: { xPx: 10, yPx: 20 } }),
    null,
  );
});

test("delta transform is an explicit snapping/constraint hook", () => {
  const drag = controller({
    transformDelta: ({ deltaMm }) => ({
      x: Math.round(deltaMm.x / 5) * 5,
      y: Math.round(deltaMm.y / 5) * 5,
    }),
  });
  drag.begin({
    pointerId: 1,
    screenPoint: { xPx: 0, yPx: 0 },
    elementIds: ["a"],
  });

  assert.deepEqual(
    drag.update({ pointerId: 1, screenPoint: { xPx: 47, yPx: 62 } }),
    {
      kind: "move-preview",
      pointerId: 1,
      elementIds: ["a"],
      deltaMm: { x: 5, y: 5 },
    },
  );
});

test("DOM binding captures the pointer, publishes overlays, and commits once", () => {
  const surface = new FakeSurface();
  const drag = controller();
  const overlays = [];
  const commits = [];
  const dispose = bindMovePointerSurface(surface, {
    controller: drag,
    resolveElementIds: () => ["a", "b"],
    onOverlay: (overlay) => overlays.push(overlay),
    onCommit: (commit) => commits.push(commit),
  });

  const down = surface.dispatch("pointerdown", {
    pointerId: 9,
    clientX: 100,
    clientY: 100,
  });
  assert.equal(down.prevented, true);
  assert.equal(surface.hasPointerCapture(9), true);

  const move = surface.dispatch("pointermove", {
    pointerId: 9,
    clientX: 130,
    clientY: 80,
  });
  assert.equal(move.prevented, true);
  assert.deepEqual(overlays.at(-1), {
    kind: "move-preview",
    pointerId: 9,
    elementIds: ["a", "b"],
    deltaMm: { x: 3, y: -2 },
  });

  const up = surface.dispatch("pointerup", {
    pointerId: 9,
    clientX: 140,
    clientY: 90,
  });
  assert.equal(up.prevented, true);
  assert.equal(surface.hasPointerCapture(9), false);
  assert.equal(overlays.at(-1), null);
  assert.deepEqual(commits, [
    {
      kind: "move-elements",
      elementIds: ["a", "b"],
      deltaMm: { x: 4, y: -1 },
    },
  ]);

  dispose();
});

test("pointercancel and lostpointercapture clear overlays without commit", () => {
  for (const terminalEvent of ["pointercancel", "lostpointercapture"]) {
    const surface = new FakeSurface();
    const drag = controller();
    const overlays = [];
    const commits = [];
    bindMovePointerSurface(surface, {
      controller: drag,
      resolveElementIds: () => ["a"],
      onOverlay: (overlay) => overlays.push(overlay),
      onCommit: (commit) => commits.push(commit),
    });

    surface.dispatch("pointerdown", { pointerId: 5 });
    surface.dispatch("pointermove", {
      pointerId: 5,
      clientX: 20,
      clientY: 10,
    });
    surface.dispatch(terminalEvent, { pointerId: 5 });

    assert.equal(drag.isActive, false, terminalEvent);
    assert.equal(overlays.at(-1), null, terminalEvent);
    assert.deepEqual(commits, [], terminalEvent);
  }
});

test("invalid interaction inputs fail before a gesture can escape the frontend", () => {
  assert.throws(
    () => new MoveGestureController(),
    InteractionContractError,
  );

  const drag = controller();
  assert.throws(
    () =>
      drag.begin({
        pointerId: -1,
        screenPoint: { xPx: 0, yPx: 0 },
        elementIds: ["a"],
      }),
    InteractionContractError,
  );
  assert.throws(
    () =>
      drag.begin({
        pointerId: 1,
        screenPoint: { xPx: 0, yPx: 0 },
        elementIds: [],
      }),
    InteractionContractError,
  );
});
