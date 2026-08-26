const Z_ORDER_OPERATIONS = new Set([
  'bringToFront',
  'sendToBack',
  'bringForward',
  'sendBackward',
]);

export function createZOrderRequest(operation) {
  if (!Z_ORDER_OPERATIONS.has(operation)) {
    throw new TypeError(`Unsupported z-order operation: ${String(operation)}`);
  }
  return { operation };
}

export function isZOrderActionEnabled({
  selectionCount = 0,
  layerVisible = false,
  layerLocked = true,
  busy = false,
} = {}) {
  return !busy && Number(selectionCount) > 0 && layerVisible === true && layerLocked !== true;
}
