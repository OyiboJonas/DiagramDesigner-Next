const NOOP = () => {};
const CONNECTOR_KINDS = new Set(['straight', 'orthogonal']);

export class ConnectorGestureError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ConnectorGestureError';
  }
}

/**
 * Frontend-only connector drawing state.
 *
 * Raw pointer movement and the transient preview remain inside the WebView. The
 * controller emits exactly one semantic create intent when a sufficiently long
 * gesture finishes. It never mutates document/editor state and knows nothing
 * about Tauri IPC or DDNX persistence.
 */
export class ConnectorGestureController {
  #screenToDocument;
  #minimumLengthMm;
  #active = null;

  constructor({ screenToDocument, minimumLengthMm = 0.5 } = {}) {
    if (typeof screenToDocument !== 'function') {
      throw new ConnectorGestureError('screenToDocument must be a function');
    }
    if (!Number.isFinite(minimumLengthMm) || minimumLengthMm < 0) {
      throw new ConnectorGestureError('minimumLengthMm must be finite and non-negative');
    }
    this.#screenToDocument = screenToDocument;
    this.#minimumLengthMm = minimumLengthMm;
  }

  get activePointerId() {
    return this.#active?.pointerId ?? null;
  }

  get isActive() {
    return this.#active !== null;
  }

  begin({ pointerId, screenPoint, connectorKind }) {
    if (this.#active !== null) {
      throw new ConnectorGestureError('a connector gesture is already active');
    }
    validatePointerId(pointerId);
    const kind = normalizeConnectorKind(connectorKind);
    const startMm = validateDocumentPoint(this.#screenToDocument(validateScreenPoint(screenPoint)));
    this.#active = {
      pointerId,
      connectorKind: kind,
      startMm,
      endMm: startMm,
    };
    return this.#preview();
  }

  update({ pointerId, screenPoint }) {
    if (this.#active === null || this.#active.pointerId !== pointerId) {
      return null;
    }
    this.#active.endMm = validateDocumentPoint(
      this.#screenToDocument(validateScreenPoint(screenPoint)),
    );
    return this.#preview();
  }

  finish({ pointerId, screenPoint }) {
    if (this.#active === null || this.#active.pointerId !== pointerId) {
      return null;
    }
    this.update({ pointerId, screenPoint });
    const finished = this.#active;
    this.#active = null;
    if (distance(finished.startMm, finished.endMm) < this.#minimumLengthMm) {
      return null;
    }
    return Object.freeze({
      kind: 'create-connector',
      connectorKind: finished.connectorKind,
      startMm: Object.freeze({ ...finished.startMm }),
      endMm: Object.freeze({ ...finished.endMm }),
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
      kind: 'connector-preview',
      pointerId: this.#active.pointerId,
      connectorKind: this.#active.connectorKind,
      startMm: Object.freeze({ ...this.#active.startMm }),
      endMm: Object.freeze({ ...this.#active.endMm }),
    });
  }
}

/** Bind browser Pointer Events to one connector controller. */
export function bindConnectorPointerSurface(
  surface,
  {
    controller,
    getConnectorKind,
    onOverlay = NOOP,
    onCommit = NOOP,
    onError = (error) => {
      throw error;
    },
  } = {},
) {
  validateSurface(surface);
  if (!(controller instanceof ConnectorGestureController)) {
    throw new ConnectorGestureError('controller must be a ConnectorGestureController');
  }
  if (typeof getConnectorKind !== 'function') {
    throw new ConnectorGestureError('getConnectorKind must be a function');
  }
  for (const [name, callback] of [
    ['onOverlay', onOverlay],
    ['onCommit', onCommit],
    ['onError', onError],
  ]) {
    if (typeof callback !== 'function') {
      throw new ConnectorGestureError(`${name} must be a function`);
    }
  }

  const listeners = {
    pointerdown: (event) => {
      if (event.button !== 0 || event.isPrimary === false || controller.isActive) {
        return;
      }
      const requestedKind = getConnectorKind();
      if (requestedKind == null) {
        return;
      }
      try {
        const overlay = controller.begin({
          pointerId: event.pointerId,
          screenPoint: eventPoint(event),
          connectorKind: requestedKind,
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
      // Browser teardown may already have released capture.
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

/**
 * Match the current Rust renderer's free-endpoint orthogonal routing convention.
 * Keeping this helper frontend-only means preview geometry never becomes document
 * state; Rust remains authoritative after commit.
 */
export function buildOrthogonalPreviewPoints(startMm, endMm) {
  const start = validateDocumentPoint(startMm);
  const end = validateDocumentPoint(endMm);
  const midpoint = {
    x: (start.x + end.x) / 2,
    y: (start.y + end.y) / 2,
  };
  const horizontal = Math.abs(start.x - end.x) > Math.abs(start.y - end.y);
  const points = horizontal
    ? [
        start,
        { x: midpoint.x, y: start.y },
        { x: midpoint.x, y: midpoint.y },
        { x: midpoint.x, y: end.y },
        end,
      ]
    : [
        start,
        { x: start.x, y: midpoint.y },
        { x: midpoint.x, y: midpoint.y },
        { x: end.x, y: midpoint.y },
        end,
      ];
  const deduped = [];
  for (const point of points) {
    const previous = deduped.at(-1);
    if (!previous || previous.x !== point.x || previous.y !== point.y) {
      deduped.push(Object.freeze({ ...point }));
    }
  }
  return Object.freeze(deduped);
}

export function normalizeConnectorKind(value) {
  if (!CONNECTOR_KINDS.has(value)) {
    throw new ConnectorGestureError(`unsupported connector kind: ${String(value)}`);
  }
  return value;
}

function validateSurface(surface) {
  for (const name of [
    'addEventListener',
    'removeEventListener',
    'setPointerCapture',
    'releasePointerCapture',
  ]) {
    if (typeof surface?.[name] !== 'function') {
      throw new ConnectorGestureError(`surface.${name} must be a function`);
    }
  }
}

function eventPoint(event) {
  return validateScreenPoint({ xPx: event.clientX, yPx: event.clientY });
}

function validatePointerId(pointerId) {
  if (!Number.isInteger(pointerId) || pointerId < 0) {
    throw new ConnectorGestureError('pointerId must be a non-negative integer');
  }
}

function validateScreenPoint(point) {
  const normalized = { xPx: point?.xPx, yPx: point?.yPx };
  if (!Number.isFinite(normalized.xPx) || !Number.isFinite(normalized.yPx)) {
    throw new ConnectorGestureError('screen point must contain finite xPx/yPx');
  }
  return normalized;
}

function validateDocumentPoint(point) {
  const normalized = { x: point?.x, y: point?.y };
  if (!Number.isFinite(normalized.x) || !Number.isFinite(normalized.y)) {
    throw new ConnectorGestureError('document point must contain finite x/y');
  }
  return normalized;
}

function distance(left, right) {
  return Math.hypot(right.x - left.x, right.y - left.y);
}
