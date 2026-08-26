export function isClipboardSelectionActionEnabled({ selectionCount = 0, busy = false } = {}) {
  return !busy && Number(selectionCount) > 0;
}
