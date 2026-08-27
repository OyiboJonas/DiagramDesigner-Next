const ARRANGE_MINIMUMS = new Map([
  ['alignLeft', 2],
  ['alignHorizontalCenter', 2],
  ['alignRight', 2],
  ['alignTop', 2],
  ['alignVerticalCenter', 2],
  ['alignBottom', 2],
  ['distributeHorizontal', 3],
  ['distributeVertical', 3],
]);

export function arrangeMinimumSelection(operation) {
  const minimum = ARRANGE_MINIMUMS.get(operation);
  if (minimum === undefined) {
    throw new TypeError(`Unsupported align/distribute operation: ${String(operation)}`);
  }
  return minimum;
}

export function createArrangeRequest(operation) {
  arrangeMinimumSelection(operation);
  return { operation };
}

export function isArrangeActionEnabled({
  operation,
  selectionCount = 0,
  layerVisible = false,
  layerLocked = true,
  busy = false,
} = {}) {
  const minimum = arrangeMinimumSelection(operation);
  return (
    !busy &&
    Number(selectionCount) >= minimum &&
    layerVisible === true &&
    layerLocked !== true
  );
}
