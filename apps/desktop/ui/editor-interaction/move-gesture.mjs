const NOOP = () => {};

export class InteractionContractError extends Error {
  constructor(message) {
    super(message);
    this.name = "InteractionContractError";
  }
}

/**
 * Frontend-only move gesture state.
 *
 * Raw pointer movement must remain local to the WebView. This controller emits
 * transient overlays while a gesture is active and exactly one semantic move
 * intent when the gesture finishes. It never mutates editor/document state and
 * does not know about Tauri IPC, DDNX, Undo/Redo, SVG DOM structure, or Rust
 * command internals.
 */
export class MoveGestureController {
  #screenToDocument;
  #transformDelta;
  #active = null;

  constructor({ screenToDocument, transformDelta = ({ deltaMm }) => deltaMm } = {}) {
    if (typeof screenToDocument !== "function") {
      throw new InteractionContractError("screenToDocument must be a function");
    }
    if (typeof transformDelta !== "function") {
      throw new InteractionContractError("transformDelta must be a function");
    }
    this.#screenToDocument = screenToDocument;
    this.#transformDelta = transformDelta;
  }

  get activePointerId() {
    return this.#active?.pointerId ?? null;
  }

  get isActive() {
    return this.#active !== null;
  }

  begin({ pointerId, screenPoint, elementIds }) {
    if (this.#active !== null) {
      throw new InteractionContractError("a move gesture is already active");
    }
    validatePointerId(pointerId);
    const startScreen = validateScreenPoint(screenPoint);
    const ids = normalizeElementIds(elementIds);
    const startMm = validateDocumentPoint(this.#screenToDocument(startScreen));

    this.#active = {
      pointerId,
      elementIds: ids,
      startMm,
      currentMm: startMm,
      deltaMm: { x: 0, y: 0 },
    };
    return this.#overlay();
  }

  update({ pointerId, screenPoint }) {
    if (this.#active === null || this.#active.pointerId !== pointerId) {
      return null;
    }
    const currentScreen = validateScreenPoint(screenPoint);
    const currentMm = validateDocumentPoint(this.#screenToDocument(currentScreen));
    const rawDeltaMm = {
      x: currentMm.x - this.#active.startMm.x,
      y: currentMm.y - this.#active.startMm.y,
    };
    const deltaMm = validateDelta(
      this.#transformDelta({
        deltaMm: rawDeltaMm,
        startMm: { ...this.#active.startMm },
        currentMm: { ...currentMm },
        elementIds: [...this.#active.elementIds],
      }),
    );

    this.#active.currentMm = currentMm;
    this.#active.deltaMm = deltaMm;
    return this.#overlay();
  }

  finish({ pointerId, screenPoint }) {
    if (this.#active === null || this.#active.pointerId !== pointerId) {
      return null;
    }
    this.update({ pointerId, screenPoint });
    const finished = this.#active;
    this.#active = null;

    if (finished.deltaMm.x === 0 && finished.deltaMm.y === 0) {
      return null;
    }

    return Object.freeze({
      kind: "move-elements",
      elementIds: Object.freeze([...finished.elementIds]),
      deltaMm: Object.freeze({ ...finished.deltaMm }),
    });
  }

  cancel(pointerId = this.#active?.pointerId ?? null) {
    if (this.#active === null || this.#active.pointerId !== pointerId) {
      return false;
    }
    this.#active = null;
    return true;
  }

  #overlay() {
    if (this.#active === null) {
      return null;
    }
    return Object.freeze({
      kind: "move-preview",
      pointerId: this.#active.pointerId,
      elementIds: Object.freeze([...this.#active.elementIds]),
      deltaMm: Object.freeze({ ...this.#active.deltaMm }),
    });
  }
}

/**
 * Bind browser Pointer Events to a move controller.
 *
 * `resolveElementIds` performs renderer/application-specific hit testing and
 * selection policy. The binding owns pointer capture after a successful begin,
 * publishes transient overlays on pointermove, and publishes one semantic
 * commit on pointerup. pointercancel/lostpointercapture never commit.
 */
export function bindMovePointerSurface(
  surface,
  {
    controller,
    resolveElementIds,
    onOverlay = NOOP,
    onCommit = NOOP,
    onError = (error) => {
      throw error;
    },
  } = {},
) {
  validateSurface(surface);
  if (!(controller instanceof MoveGestureController)) {
    throw new InteractionContractError("controller must be a MoveGestureController");
  }
  if (typeof resolveElementIds !== "function") {
    throw new InteractionContractError("resolveElementIds must be a function");
  }
  for (const [name, callback] of [
    ["onOverlay", onOverlay],
    ["onCommit", onCommit],
    ["onError", onError],
  ]) {
    if (typeof callback !== "function") {
      throw new InteractionContractError(`${name} must be a function`);
    }
  }

  const listeners = {
    pointerdown: (event) => {
      if (event.button !== 0 || event.isPrimary === false || controller.isActive) {
        return;
      }
      let elementIds;
      try {
        elementIds = resolveElementIds(event);
      } catch (error) {
        onError(error);
        return;
      }
      if (elementIds == null) {
        return;
      }

      try {
        const overlay = controller.begin({
          pointerId: event.pointerId,
          screenPoint: eventPoint(event),
          elementIds,
        });
        surface.setPointerCapture(event.pointerId);
        event.preventDefault?.();
        onOverlay(overlay);
      } catch (error) {
        controller.cancel(event.pointerId);
        onOverlay(null);
        onError(error);
      }
    },

    pointermove: (event) => {
      if (controller.activePointerId !== event.pointerId) {
        return;
      }
      try {
        const overlay = controller.update({
          pointerId: event.pointerId,
          screenPoint: eventPoint(event),
        });
        if (overlay !== null) {
          event.preventDefault?.();
          onOverlay(overlay);
        }
      } catch (error) {
        cancelCaptured(event.pointerId);
        onError(error);
      }
    },

    pointerup: (event) => {
      if (controller.activePointerId !== event.pointerId) {
        return;
      }
      try {
        const commit = controller.finish({
          pointerId: event.pointerId,
          screenPoint: eventPoint(event),
        });
        releaseCapture(event.pointerId);
        event.preventDefault?.();
        onOverlay(null);
        if (commit !== null) {
          onCommit(commit);
        }
      } catch (error) {
        cancelCaptured(event.pointerId);
        onError(error);
      }
    },

    pointercancel: (event) => {
      if (controller.activePointerId !== event.pointerId) {
        return;
      }
      cancelCaptured(event.pointerId);
    },

    lostpointercapture: (event) => {
      if (controller.cancel(event.pointerId)) {
        onOverlay(null);
      }
    },
  };

  function releaseCapture(pointerId) {
    try {
      if (surface.hasPointerCapture?.(pointerId) !== false) {
        surface.releasePointerCapture(pointerId);
      }
    } catch {
      // The browser may already have released capture during teardown. The
      // semantic gesture has already been resolved, so there is nothing else to
      // recover here.
    }
  }

  function cancelCaptured(pointerId) {
    const cancelled = controller.cancel(pointerId);
    releaseCapture(pointerId);
    if (cancelled) {
      onOverlay(null);
    }
  }

  for (const [type, listener] of Object.entries(listeners)) {
    surface.addEventListener(type, listener);
  }

  return () => {
    const activePointerId = controller.activePointerId;
    if (activePointerId !== null) {
      cancelCaptured(activePointerId);
    }
    for (const [type, listener] of Object.entries(listeners)) {
      surface.removeEventListener(type, listener);
    }
  };
}

function validateSurface(surface) {
  for (const name of [
    "addEventListener",
    "removeEventListener",
    "setPointerCapture",
    "releasePointerCapture",
  ]) {
    if (typeof surface?.[name] !== "function") {
      throw new InteractionContractError(`surface.${name} must be a function`);
    }
  }
}

function eventPoint(event) {
  return validateScreenPoint({ xPx: event.clientX, yPx: event.clientY });
}

function validatePointerId(pointerId) {
  if (!Number.isInteger(pointerId) || pointerId < 0) {
    throw new InteractionContractError("pointerId must be a non-negative integer");
  }
}

function validateScreenPoint(point) {
  const normalized = { xPx: point?.xPx, yPx: point?.yPx };
  if (!Number.isFinite(normalized.xPx) || !Number.isFinite(normalized.yPx)) {
    throw new InteractionContractError("screen point must contain finite xPx/yPx");
  }
  return normalized;
}

function validateDocumentPoint(point) {
  const normalized = { x: point?.x, y: point?.y };
  if (!Number.isFinite(normalized.x) || !Number.isFinite(normalized.y)) {
    throw new InteractionContractError("document point must contain finite x/y");
  }
  return normalized;
}

function validateDelta(delta) {
  const normalized = { x: delta?.x, y: delta?.y };
  if (!Number.isFinite(normalized.x) || !Number.isFinite(normalized.y)) {
    throw new InteractionContractError("transformed delta must contain finite x/y");
  }
  return normalized;
}

function normalizeElementIds(elementIds) {
  if (elementIds == null || typeof elementIds[Symbol.iterator] !== "function") {
    throw new InteractionContractError("elementIds must be iterable");
  }
  const ids = [];
  const seen = new Set();
  for (const id of elementIds) {
    if (typeof id !== "string" || id.length === 0) {
      throw new InteractionContractError("element IDs must be non-empty strings");
    }
    if (!seen.has(id)) {
      seen.add(id);
      ids.push(id);
    }
  }
  if (ids.length === 0) {
    throw new InteractionContractError("a move gesture requires at least one element");
  }
  return Object.freeze(ids);
}
