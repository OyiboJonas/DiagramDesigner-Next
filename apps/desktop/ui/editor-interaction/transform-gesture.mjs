const NOOP = () => {};
const HANDLE_AXES = Object.freeze({
  nw: Object.freeze({ x: -1, y: -1 }),
  n: Object.freeze({ x: 0, y: -1 }),
  ne: Object.freeze({ x: 1, y: -1 }),
  e: Object.freeze({ x: 1, y: 0 }),
  se: Object.freeze({ x: 1, y: 1 }),
  s: Object.freeze({ x: 0, y: 1 }),
  sw: Object.freeze({ x: -1, y: 1 }),
  w: Object.freeze({ x: -1, y: 0 }),
});
const ALL_HANDLES = new Set([...Object.keys(HANDLE_AXES), "rotate"]);

export class TransformGestureError extends Error {
  constructor(message) {
    super(message);
    this.name = "TransformGestureError";
  }
}

/**
 * Frontend-only direct transform state.
 *
 * The controller consumes renderer-neutral document geometry and emits transient
 * previews plus one final semantic transform intent. It never mutates editor,
 * history, persistence or SVG state.
 */
export class TransformGestureController {
  #screenToDocument;
  #minimumSizeMm;
  #rotationSnapDeg;
  #active = null;

  constructor({ screenToDocument, minimumSizeMm = 1, rotationSnapDeg = 15 } = {}) {
    if (typeof screenToDocument !== "function") {
      throw new TransformGestureError("screenToDocument must be a function");
    }
    if (!Number.isFinite(minimumSizeMm) || minimumSizeMm <= 0) {
      throw new TransformGestureError("minimumSizeMm must be finite and positive");
    }
    if (!Number.isFinite(rotationSnapDeg) || rotationSnapDeg <= 0) {
      throw new TransformGestureError("rotationSnapDeg must be finite and positive");
    }
    this.#screenToDocument = screenToDocument;
    this.#minimumSizeMm = minimumSizeMm;
    this.#rotationSnapDeg = rotationSnapDeg;
  }

  get activePointerId() {
    return this.#active?.pointerId ?? null;
  }

  get isActive() {
    return this.#active !== null;
  }

  begin({ pointerId, screenPoint, handle, selection }) {
    if (this.#active !== null) {
      throw new TransformGestureError("a transform gesture is already active");
    }
    validatePointerId(pointerId);
    const normalizedHandle = normalizeTransformHandle(handle);
    const normalizedSelection = normalizeSelection(selection);
    const startMm = validateDocumentPoint(
      this.#screenToDocument(validateScreenPoint(screenPoint)),
    );
    const centerMm = boundsCenter(normalizedSelection.boundsMm);

    this.#active = {
      pointerId,
      handle: normalizedHandle,
      elementId: normalizedSelection.elementId,
      originalBoundsMm: normalizedSelection.boundsMm,
      originalRotationDeg: normalizedSelection.rotationDeg,
      pageSize: normalizedSelection.pageSize,
      centerMm,
      startAngleDeg: angleDeg(centerMm, startMm),
      boundsMm: normalizedSelection.boundsMm,
      rotationDeg: normalizedSelection.rotationDeg,
    };
    return this.#preview();
  }

  update({ pointerId, screenPoint, shiftKey = false }) {
    if (this.#active === null || this.#active.pointerId !== pointerId) {
      return null;
    }
    const currentMm = validateDocumentPoint(
      this.#screenToDocument(validateScreenPoint(screenPoint)),
    );

    if (this.#active.handle === "rotate") {
      const currentAngleDeg = angleDeg(this.#active.centerMm, currentMm);
      const deltaDeg = normalizeSignedDegrees(currentAngleDeg - this.#active.startAngleDeg);
      let rotationDeg = normalizeSignedDegrees(
        this.#active.originalRotationDeg + deltaDeg,
      );
      if (shiftKey) {
        rotationDeg = normalizeSignedDegrees(
          Math.round(rotationDeg / this.#rotationSnapDeg) * this.#rotationSnapDeg,
        );
      }
      this.#active.rotationDeg = rotationDeg;
      this.#active.boundsMm = this.#active.originalBoundsMm;
    } else {
      this.#active.boundsMm = resizeRotatedBounds({
        boundsMm: this.#active.originalBoundsMm,
        rotationDeg: this.#active.originalRotationDeg,
        handle: this.#active.handle,
        pointerMm: currentMm,
        minimumSizeMm: this.#minimumSizeMm,
        pageSize: this.#active.pageSize,
      });
      this.#active.rotationDeg = this.#active.originalRotationDeg;
    }
    return this.#preview();
  }

  finish({ pointerId, screenPoint, shiftKey = false }) {
    if (this.#active === null || this.#active.pointerId !== pointerId) {
      return null;
    }
    this.update({ pointerId, screenPoint, shiftKey });
    const finished = this.#active;
    this.#active = null;

    if (
      sameBounds(finished.boundsMm, finished.originalBoundsMm) &&
      nearlyEqual(finished.rotationDeg, finished.originalRotationDeg)
    ) {
      return null;
    }

    return Object.freeze({
      kind: "transform-element",
      elementId: finished.elementId,
      boundsMm: freezeBounds(finished.boundsMm),
      rotationDeg: finished.rotationDeg,
    });
  }

  cancel(pointerId = this.#active?.pointerId ?? null) {
    if (this.#active === null || this.#active.pointerId !== pointerId) {
      return false;
    }
    this.#active = null;
    return true;
  }

  #preview() {
    if (this.#active === null) {
      return null;
    }
    return Object.freeze({
      kind: "transform-preview",
      pointerId: this.#active.pointerId,
      handle: this.#active.handle,
      elementId: this.#active.elementId,
      boundsMm: freezeBounds(this.#active.boundsMm),
      rotationDeg: this.#active.rotationDeg,
    });
  }
}

export function bindTransformPointerSurface(
  surface,
  {
    controller,
    resolveHandle,
    onOverlay = NOOP,
    onCommit = NOOP,
    onError = (error) => {
      throw error;
    },
  } = {},
) {
  validateSurface(surface);
  if (!(controller instanceof TransformGestureController)) {
    throw new TransformGestureError("controller must be a TransformGestureController");
  }
  if (typeof resolveHandle !== "function") {
    throw new TransformGestureError("resolveHandle must be a function");
  }
  for (const [name, callback] of [
    ["onOverlay", onOverlay],
    ["onCommit", onCommit],
    ["onError", onError],
  ]) {
    if (typeof callback !== "function") {
      throw new TransformGestureError(`${name} must be a function`);
    }
  }

  const listeners = {
    pointerdown: (event) => {
      if (event.button !== 0 || event.isPrimary === false || controller.isActive) {
        return;
      }
      let resolved;
      try {
        resolved = resolveHandle(event);
      } catch (error) {
        onError(error);
        return;
      }
      if (resolved == null) {
        return;
      }
      try {
        const preview = controller.begin({
          pointerId: event.pointerId,
          screenPoint: eventPoint(event),
          handle: resolved.handle,
          selection: resolved.selection,
        });
        surface.setPointerCapture(event.pointerId);
        event.preventDefault?.();
        onOverlay(preview);
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
        const preview = controller.update({
          pointerId: event.pointerId,
          screenPoint: eventPoint(event),
          shiftKey: event.shiftKey,
        });
        if (preview !== null) {
          event.preventDefault?.();
          onOverlay(preview);
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
          shiftKey: event.shiftKey,
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
      if (controller.activePointerId === event.pointerId) {
        cancelCaptured(event.pointerId);
      }
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
      // Pointer capture can already be gone during teardown.
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
    const pointerId = controller.activePointerId;
    if (pointerId !== null) {
      cancelCaptured(pointerId);
    }
    for (const [type, listener] of Object.entries(listeners)) {
      surface.removeEventListener(type, listener);
    }
  };
}

export function normalizeTransformHandle(handle) {
  if (typeof handle !== "string" || !ALL_HANDLES.has(handle)) {
    throw new TransformGestureError(`unsupported transform handle: ${String(handle)}`);
  }
  return handle;
}

export function resizeRotatedBounds({
  boundsMm,
  rotationDeg,
  handle,
  pointerMm,
  minimumSizeMm = 1,
  pageSize = null,
} = {}) {
  const bounds = normalizeBounds(boundsMm);
  if (!Number.isFinite(rotationDeg)) {
    throw new TransformGestureError("rotationDeg must be finite");
  }
  const axes = HANDLE_AXES[normalizeTransformHandle(handle)];
  if (!axes) {
    throw new TransformGestureError("rotate handle cannot resize bounds");
  }
  if (!Number.isFinite(minimumSizeMm) || minimumSizeMm <= 0) {
    throw new TransformGestureError("minimumSizeMm must be finite and positive");
  }
  const point = validateDocumentPoint(pointerMm);
  const center = boundsCenter(bounds);
  const local = rotateVector(
    { x: point.x - center.x, y: point.y - center.y },
    -rotationDeg,
  );

  let left = -bounds.width / 2;
  let right = bounds.width / 2;
  let top = -bounds.height / 2;
  let bottom = bounds.height / 2;

  if (axes.x < 0) {
    left = Math.min(local.x, right - minimumSizeMm);
  } else if (axes.x > 0) {
    right = Math.max(local.x, left + minimumSizeMm);
  }
  if (axes.y < 0) {
    top = Math.min(local.y, bottom - minimumSizeMm);
  } else if (axes.y > 0) {
    bottom = Math.max(local.y, top + minimumSizeMm);
  }

  const width = right - left;
  const height = bottom - top;
  const localCenterOffset = {
    x: (left + right) / 2,
    y: (top + bottom) / 2,
  };
  const centerOffset = rotateVector(localCenterOffset, rotationDeg);
  const resized = {
    x: center.x + centerOffset.x - width / 2,
    y: center.y + centerOffset.y - height / 2,
    width,
    height,
  };
  return freezeBounds(clampBoundsToPage(resized, pageSize));
}

function normalizeSelection(selection) {
  if (typeof selection?.elementId !== "string" || selection.elementId.length === 0) {
    throw new TransformGestureError("selection requires a non-empty elementId");
  }
  if (!Number.isFinite(selection.rotationDeg)) {
    throw new TransformGestureError("selection rotationDeg must be finite");
  }
  return Object.freeze({
    elementId: selection.elementId,
    boundsMm: normalizeBounds(selection.boundsMm),
    rotationDeg: selection.rotationDeg,
    pageSize: normalizePageSize(selection.pageSize),
  });
}

function normalizeBounds(bounds) {
  const normalized = {
    x: bounds?.x,
    y: bounds?.y,
    width: bounds?.width,
    height: bounds?.height,
  };
  if (
    !Number.isFinite(normalized.x) ||
    !Number.isFinite(normalized.y) ||
    !Number.isFinite(normalized.width) ||
    !Number.isFinite(normalized.height) ||
    normalized.width <= 0 ||
    normalized.height <= 0
  ) {
    throw new TransformGestureError("bounds must contain finite positive geometry");
  }
  return freezeBounds(normalized);
}

function normalizePageSize(pageSize) {
  if (pageSize == null) {
    return null;
  }
  const normalized = { width: pageSize.width, height: pageSize.height };
  if (
    !Number.isFinite(normalized.width) ||
    !Number.isFinite(normalized.height) ||
    normalized.width <= 0 ||
    normalized.height <= 0
  ) {
    throw new TransformGestureError("pageSize must contain finite positive geometry");
  }
  return Object.freeze(normalized);
}

function clampBoundsToPage(bounds, pageSize) {
  if (pageSize == null) {
    return bounds;
  }
  const width = Math.min(bounds.width, pageSize.width);
  const height = Math.min(bounds.height, pageSize.height);
  return {
    x: Math.min(Math.max(bounds.x, 0), pageSize.width - width),
    y: Math.min(Math.max(bounds.y, 0), pageSize.height - height),
    width,
    height,
  };
}

function boundsCenter(bounds) {
  return {
    x: bounds.x + bounds.width / 2,
    y: bounds.y + bounds.height / 2,
  };
}

function angleDeg(center, point) {
  return (Math.atan2(point.y - center.y, point.x - center.x) * 180) / Math.PI;
}

function rotateVector(vector, degrees) {
  const radians = (degrees * Math.PI) / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  return {
    x: vector.x * cos - vector.y * sin,
    y: vector.x * sin + vector.y * cos,
  };
}

function normalizeSignedDegrees(value) {
  let normalized = ((value + 180) % 360 + 360) % 360 - 180;
  if (Object.is(normalized, -0)) {
    normalized = 0;
  }
  return normalized;
}

function sameBounds(left, right) {
  return (
    nearlyEqual(left.x, right.x) &&
    nearlyEqual(left.y, right.y) &&
    nearlyEqual(left.width, right.width) &&
    nearlyEqual(left.height, right.height)
  );
}

function nearlyEqual(left, right) {
  return Math.abs(left - right) <= 1e-9;
}

function freezeBounds(bounds) {
  return Object.freeze({
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
  });
}

function validateSurface(surface) {
  for (const name of [
    "addEventListener",
    "removeEventListener",
    "setPointerCapture",
    "releasePointerCapture",
  ]) {
    if (typeof surface?.[name] !== "function") {
      throw new TransformGestureError(`surface.${name} must be a function`);
    }
  }
}

function eventPoint(event) {
  return validateScreenPoint({ xPx: event.clientX, yPx: event.clientY });
}

function validatePointerId(pointerId) {
  if (!Number.isInteger(pointerId) || pointerId < 0) {
    throw new TransformGestureError("pointerId must be a non-negative integer");
  }
}

function validateScreenPoint(point) {
  const normalized = { xPx: point?.xPx, yPx: point?.yPx };
  if (!Number.isFinite(normalized.xPx) || !Number.isFinite(normalized.yPx)) {
    throw new TransformGestureError("screen point must contain finite xPx/yPx");
  }
  return normalized;
}

function validateDocumentPoint(point) {
  const normalized = { x: point?.x, y: point?.y };
  if (!Number.isFinite(normalized.x) || !Number.isFinite(normalized.y)) {
    throw new TransformGestureError("document point must contain finite x/y");
  }
  return normalized;
}
