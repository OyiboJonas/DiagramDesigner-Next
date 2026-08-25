import assert from "node:assert/strict";
import test from "node:test";

import {
  ConnectorEndpointGestureController,
  ConnectorEndpointGestureError,
  bindConnectorEndpointPointerSurface,
  nearestPortTarget,
} from "./connector-endpoint-gesture.mjs";

function controller(options = {}) {
  return new ConnectorEndpointGestureController({
    screenToDocument: ({ xPx, yPx }) => ({ x: xPx / 10, y: yPx / 10 }),
    resolvePortTarget: () => null,
    ...options,
  });
}

function endpoint(positionMm, connection = null) {
  return { positionMm, connection };
}

function beginEndpointDrag(drag, overrides = {}) {
  return drag.begin({
    pointerId: 7,
    screenPoint: { xPx: 20, yPx: 30 },
    elementId: "connector-a",
    side: "start",
    connectorKind: "straight",
    startEndpoint: endpoint({ x: 2, y: 3 }),
    endEndpoint: endpoint({ x: 12, y: 8 }),
    ...overrides,
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

test("endpoint drag stays transient and commits one free endpoint", () => {
  const drag = controller();
  assert.deepEqual(beginEndpointDrag(drag), {
    kind: "connector-endpoint-preview",
    pointerId: 7,
    elementId: "connector-a",
    side: "start",
    connectorKind: "straight",
    startMm: { x: 2, y: 3 },
    endMm: { x: 12, y: 8 },
    positionMm: { x: 2, y: 3 },
    connection: null,
  });

  assert.deepEqual(
    drag.update({ pointerId: 7, screenPoint: { xPx: 45, yPx: 70 } }),
    {
      kind: "connector-endpoint-preview",
      pointerId: 7,
      elementId: "connector-a",
      side: "start",
      connectorKind: "straight",
      startMm: { x: 4.5, y: 7 },
      endMm: { x: 12, y: 8 },
      positionMm: { x: 4.5, y: 7 },
      connection: null,
    },
  );

  assert.deepEqual(
    drag.finish({ pointerId: 7, screenPoint: { xPx: 50, yPx: 80 } }),
    {
      kind: "set-connector-endpoint",
      elementId: "connector-a",
      side: "start",
      positionMm: { x: 5, y: 8 },
      connection: null,
    },
  );
  assert.equal(drag.isActive, false);
});

test("nearest eligible port becomes both preview position and durable connection", () => {
  const ports = [
    { elementId: "shape-a", portId: "port-a", positionMm: { x: 5, y: 5 } },
    { elementId: "shape-b", portId: "port-b", positionMm: { x: 5.4, y: 5 } },
  ];
  const drag = controller({
    resolvePortTarget: ({ pointMm, elementId }) =>
      nearestPortTarget(pointMm, ports, 0.6, { excludeElementId: elementId }),
  });
  beginEndpointDrag(drag);

  const preview = drag.update({ pointerId: 7, screenPoint: { xPx: 52, yPx: 50 } });
  assert.deepEqual(preview.positionMm, { x: 5, y: 5 });
  assert.deepEqual(preview.connection, { elementId: "shape-a", portId: "port-a" });

  const commit = drag.finish({ pointerId: 7, screenPoint: { xPx: 52, yPx: 50 } });
  assert.deepEqual(commit, {
    kind: "set-connector-endpoint",
    elementId: "connector-a",
    side: "start",
    positionMm: { x: 5, y: 5 },
    connection: { elementId: "shape-a", portId: "port-a" },
  });
});

test("dropping away from a port detaches an initially connected endpoint", () => {
  const drag = controller();
  beginEndpointDrag(drag, {
    side: "end",
    connectorKind: "orthogonal",
    endEndpoint: endpoint(
      { x: 12, y: 8 },
      { elementId: "shape-a", portId: "port-a" },
    ),
  });

  const commit = drag.finish({ pointerId: 7, screenPoint: { xPx: 140, yPx: 90 } });
  assert.deepEqual(commit, {
    kind: "set-connector-endpoint",
    elementId: "connector-a",
    side: "end",
    positionMm: { x: 14, y: 9 },
    connection: null,
  });
});

test("nearestPortTarget respects threshold and source exclusion", () => {
  const ports = [
    { elementId: "connector-a", portId: "self", positionMm: { x: 1, y: 1 } },
    { elementId: "shape-a", portId: "far", positionMm: { x: 4, y: 4 } },
    { elementId: "shape-b", portId: "near", positionMm: { x: 2.1, y: 2 } },
  ];
  assert.deepEqual(
    nearestPortTarget({ x: 2, y: 2 }, ports, 0.2, { excludeElementId: "connector-a" }),
    { elementId: "shape-b", portId: "near", positionMm: { x: 2.1, y: 2 } },
  );
  assert.equal(
    nearestPortTarget({ x: 2, y: 2 }, ports, 0.05, { excludeElementId: "connector-a" }),
    null,
  );
});

test("DOM binding captures, previews and emits exactly one endpoint commit", () => {
  const surface = new FakeSurface();
  const drag = controller();
  const overlays = [];
  const commits = [];
  const dispose = bindConnectorEndpointPointerSurface(surface, {
    controller: drag,
    resolveHandle: () => ({
      elementId: "connector-a",
      side: "start",
      connectorKind: "straight",
      startEndpoint: endpoint({ x: 2, y: 3 }),
      endEndpoint: endpoint({ x: 12, y: 8 }),
    }),
    onOverlay: (overlay) => overlays.push(overlay),
    onCommit: (commit) => commits.push(commit),
  });

  const down = surface.dispatch("pointerdown", {
    pointerId: 9,
    clientX: 20,
    clientY: 30,
  });
  assert.equal(down.prevented, true);
  assert.equal(surface.hasPointerCapture(9), true);

  surface.dispatch("pointermove", { pointerId: 9, clientX: 50, clientY: 60 });
  const up = surface.dispatch("pointerup", { pointerId: 9, clientX: 60, clientY: 70 });
  assert.equal(up.prevented, true);
  assert.equal(surface.hasPointerCapture(9), false);
  assert.equal(overlays.at(-1), null);
  assert.deepEqual(commits, [
    {
      kind: "set-connector-endpoint",
      elementId: "connector-a",
      side: "start",
      positionMm: { x: 6, y: 7 },
      connection: null,
    },
  ]);

  dispose();
});

test("cancel and lost pointer capture clear transient state without commit", () => {
  for (const terminalEvent of ["pointercancel", "lostpointercapture"]) {
    const surface = new FakeSurface();
    const drag = controller();
    const overlays = [];
    const commits = [];
    bindConnectorEndpointPointerSurface(surface, {
      controller: drag,
      resolveHandle: () => ({
        elementId: "connector-a",
        side: "start",
        connectorKind: "straight",
        startEndpoint: endpoint({ x: 2, y: 3 }),
        endEndpoint: endpoint({ x: 12, y: 8 }),
      }),
      onOverlay: (overlay) => overlays.push(overlay),
      onCommit: (commit) => commits.push(commit),
    });

    surface.dispatch("pointerdown", { pointerId: 5 });
    surface.dispatch("pointermove", { pointerId: 5, clientX: 40, clientY: 40 });
    surface.dispatch(terminalEvent, { pointerId: 5 });

    assert.equal(drag.isActive, false, terminalEvent);
    assert.equal(overlays.at(-1), null, terminalEvent);
    assert.deepEqual(commits, [], terminalEvent);
  }
});

test("invalid endpoint interaction data is rejected at the frontend boundary", () => {
  assert.throws(() => new ConnectorEndpointGestureController(), ConnectorEndpointGestureError);
  const drag = controller();
  assert.throws(
    () => beginEndpointDrag(drag, { side: "middle" }),
    ConnectorEndpointGestureError,
  );
  assert.throws(
    () => nearestPortTarget({ x: 0, y: 0 }, [], -1),
    ConnectorEndpointGestureError,
  );
});
