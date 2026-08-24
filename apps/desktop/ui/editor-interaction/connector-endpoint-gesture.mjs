const NOOP = () => {};
const ENDPOINT_SIDES = new Set(['start', 'end']);
const CONNECTOR_KINDS = new Set(['straight', 'orthogonal']);

export class ConnectorEndpointGestureError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ConnectorEndpointGestureError';
  }
}

/**
 * Frontend-only endpoint editing state.
 *
 * The controller performs screen/document conversion and transient target-port
 * hit testing only. Durable connection references are emitted as one semantic
 * commit and remain owned by Rust/editor-core.
 */
export class ConnectorEndpointGestureController {
  #screenToDocument;
  #resolvePortTarget;
  #active = null;

  constructor({ screenToDocument, resolvePortTarget = () => null } = {}) {
    if (typeof screenToDocument !== 'function') {
      throw new ConnectorEndpointGestureError('screenToDocument must be a function');
    }
    if (typeof resolvePortTarget !== 'function') {
      throw new ConnectorEndpointGestureError('resolvePortTarget must be a function');
    }
    this.#screenToDocument = screenToDocument;
    this.#resolvePortTarget = resolvePortTarget;
  }

  get activePointerId() {
    return this.#active?.pointerId ?? null;
  }

  get isActive() {
    return this.#active !== null;
  }

  begin({ pointerId, screenPoint, elementId, side, connectorKind, startEndpoint, endEndpoint }) {
    if (this.#active !== null) {
      throw new ConnectorEndpointGestureError('a connector endpoint gesture is already active');
    }
    validatePointerId(pointerId);
    validateScreenPoint(screenPoint);
    const normalizedElementId = validateId(elementId, 'elementId');
    const normalizedSide = normalizeEndpointSide(side);
    const normalizedKind = normalizeConnectorEndpointKind(connectorKind);
    const start = normalizeEndpoint(startEndpoint, 'startEndpoint');
    const end = normalizeEndpoint(endEndpoint, 'endEndpoint');
    this.#active = {
      pointerId,
      elementId: normalizedElementId,
      side: normalizedSide,
      connectorKind: normalizedKind,
      start,
      end,
      positionMm: normalizedSide === 'start' ? start.positionMm : end.positionMm,
      connection: normalizedSide === 'start' ? start.connection : end.connection,
    };
    return this.#preview();
  }

  update({ pointerId, screenPoint }) {
    if (this.#active === null || this.#active.pointerId !== pointerId) {
      return null;
    }
    const pointMm = validateDocumentPoint(
      this.#screenToDocument(validateScreenPoint(screenPoint)),
      'screenToDocument result',
    );
    const target = normalizePortTarget(
      this.#resolvePortTarget({
        pointMm: Object.freeze({ ...pointMm }),
        elementId: this.#active.elementId,
        side: this.#active.side,
      }),
      { allowNull: true },
    );
    this.#active.positionMm = target?.positionMm ?? pointMm;
    this.#active.connection = target
      ? Object.freeze({ elementId: target.elementId, portId: target.portId })
      : null;
    return this.#preview();
  }

  finish({ pointerId, screenPoint }) {
    if (this.#active === null || this.#active.pointerId !== pointerId) {
      return null;
    }
    this.update({ pointerId, screenPoint });
    const finished = this.#active;
    this.#active = null;
    return Object.freeze({
      kind: 'set-connector-endpoint',
      elementId: finished.elementId,
      side: finished.side,
      positionMm: Object.freeze({ ...finished.positionMm }),
      connection: finished.connection
        ? Object.freeze({ ...finished.connection })
        : null,
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
    const startMm =
      this.#active.side === 'start' ? this.#active.positionMm : this.#active.start.positionMm;
    const endMm =
      this.#active.side === 'end' ? this.#active.positionMm : this.#active.end.positionMm;
    return Object.freeze({
      kind: 'connector-endpoint-preview',
      pointerId: this.#active.pointerId,
      elementId: this.#active.elementId,
      side: this.#active.side,
      connectorKind: this.#active.connectorKind,
      startMm: Object.freeze({ ...startMm }),
      endMm: Object.freeze({ ...endMm }),
      positionMm: Object.freeze({ ...this.#active.positionMm }),
      connection: this.#active.connection
        ? Object.freeze({ ...this.#active.connection })
        : null,
    });
  }
}

/** Return the closest eligible port inside a document-space threshold. */
export function nearestPortTarget(
  pointMm,
  portTargets,
  thresholdMm,
  { excludeElementId = null } = {},
) {
  const point = validateDocumentPoint(pointMm, 'pointMm');
  if (!Number.isFinite(thresholdMm) || thresholdMm < 0) {
    throw new ConnectorEndpointGestureError('thresholdMm must be finite and non-negative');
  }
  if (!Array.isArray(portTargets)) {
    throw new ConnectorEndpointGestureError('portTargets must be an array');
  }
  let nearest = null;
  let nearestDistance = thresholdMm;
  for (const candidateValue of portTargets) {
    const candidate = normalizePortTarget(candidateValue);
    if (excludeElementId !== null && candidate.elementId === excludeElementId) {
      continue;
    }
    const candidateDistance = Math.hypot(
      candidate.positionMm.x - point.x,
      candidate.positionMm.y - point.y,
    );
    if (candidateDistance <= nearestDistance) {
      nearest = candidate;
      nearestDistance = candidateDistance;
    }
  }
  return nearest;
}

/** Bind browser Pointer Events to endpoint handles resolved by the SVG adapter. */
export function bindConnectorEndpointPointerSurface(
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
  if (!(controller instanceof ConnectorEndpointGestureController)) {
    throw new ConnectorEndpointGestureError(
      'controller must be a ConnectorEndpointGestureController',
    );
  }
  if (typeof resolveHandle !== 'function') {
    throw new ConnectorEndpointGestureError('resolveHandle must be a function');
  }
  for (const [name, callback] of [
    ['onOverlay', onOverlay],
    ['onCommit', onCommit],
    ['onError', onError],
  ]) {
    if (typeof callback !== 'function') {
      throw new ConnectorEndpointGestureError(`${name} must be a function`);
    }
  }

  const listeners = {
    pointerdown: (event) => {
      if (event.button !== 0 || event.isPrimary === false || controller.isActive) {
        return;
      }
      const handle = resolveHandle(event);
      if (!handle) {
        return;
      }
      try {
        const overlay = controller.begin({
          pointerId: event.pointerId,
          screenPoint: eventPoint(event),
          ...handle,
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

export function normalizeEndpointSide(value) {
  if (!ENDPOINT_SIDES.has(value)) {
    throw new ConnectorEndpointGestureError(`unsupported endpoint side: ${String(value)}`);
  }
  return value;
}

export function normalizeConnectorEndpointKind(value) {
  if (!CONNECTOR_KINDS.has(value)) {
    throw new ConnectorEndpointGestureError(`unsupported connector kind: ${String(value)}`);
  }
  return value;
}

function normalizeEndpoint(value, label) {
  if (!value || typeof value !== 'object') {
    throw new ConnectorEndpointGestureError(`${label} must be an endpoint object`);
  }
  return Object.freeze({
    positionMm: Object.freeze(validateDocumentPoint(value.positionMm, `${label}.positionMm`)),
    connection: normalizeConnection(value.connection, { allowNull: true }),
  });
}

function normalizePortTarget(value, { allowNull = false } = {}) {
  if (value == null && allowNull) {
    return null;
  }
  if (!value || typeof value !== 'object') {
    throw new ConnectorEndpointGestureError('port target must be an object');
  }
  return Object.freeze({
    elementId: validateId(value.elementId, 'port target elementId'),
    portId: validateId(value.portId, 'port target portId'),
    positionMm: Object.freeze(validateDocumentPoint(value.positionMm, 'port target positionMm')),
  });
}

function normalizeConnection(value, { allowNull = false } = {}) {
  if (value == null && allowNull) {
    return null;
  }
  if (!value || typeof value !== 'object') {
    throw new ConnectorEndpointGestureError('connection must be an object');
  }
  return Object.freeze({
    elementId: validateId(value.elementId, 'connection elementId'),
    portId: validateId(value.portId, 'connection portId'),
  });
}

function validateSurface(surface) {
  for (const name of [
    'addEventListener',
    'removeEventListener',
    'setPointerCapture',
    'releasePointerCapture',
  ]) {
    if (typeof surface?.[name] !== 'function') {
      throw new ConnectorEndpointGestureError(`surface.${name} must be a function`);
    }
  }
}

function eventPoint(event) {
  return validateScreenPoint({ xPx: event.clientX, yPx: event.clientY });
}

function validatePointerId(pointerId) {
  if (!Number.isInteger(pointerId) || pointerId < 0) {
    throw new ConnectorEndpointGestureError('pointerId must be a non-negative integer');
  }
}

function validateId(value, label) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new ConnectorEndpointGestureError(`${label} must be a non-empty string`);
  }
  return value;
}

function validateScreenPoint(point) {
  const normalized = { xPx: point?.xPx, yPx: point?.yPx };
  if (!Number.isFinite(normalized.xPx) || !Number.isFinite(normalized.yPx)) {
    throw new ConnectorEndpointGestureError('screen point must contain finite xPx/yPx');
  }
  return normalized;
}

function validateDocumentPoint(point, label) {
  const normalized = { x: point?.x, y: point?.y };
  if (!Number.isFinite(normalized.x) || !Number.isFinite(normalized.y)) {
    throw new ConnectorEndpointGestureError(`${label} must contain finite x/y`);
  }
  return normalized;
}
