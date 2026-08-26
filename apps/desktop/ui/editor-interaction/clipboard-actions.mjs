export function isClipboardSelectionActionEnabled({ selectionCount = 0, busy = false } = {}) {
  return !busy && Number(selectionCount) > 0;
}

export function isClipboardShortcutActionEnabled(
  { shortcut, selectionCount = 0, clipboardAvailable = false } = {},
) {
  if (shortcut === 'copy-selection' || shortcut === 'duplicate-selection') {
    return Number(selectionCount) > 0;
  }
  if (shortcut === 'paste-selection') {
    return clipboardAvailable === true;
  }
  return true;
}
