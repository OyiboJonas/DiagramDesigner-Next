export class ApplicationShortcutContractError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ApplicationShortcutContractError';
  }
}

/**
 * Resolve global desktop shortcuts without coupling the policy to DOM, Tauri or
 * editor history. Input-level Undo/Redo/Delete remains owned by editable fields;
 * Save is intentionally application-global even while a field has focus.
 */
export function resolveApplicationShortcut(
  { key, ctrlKey = false, metaKey = false, shiftKey = false, altKey = false } = {},
  { textEditing = false } = {},
) {
  if (typeof key !== 'string' || key.length === 0) {
    throw new ApplicationShortcutContractError('shortcut key must be a non-empty string');
  }
  if (altKey) {
    return null;
  }

  const command = ctrlKey || metaKey;
  const normalized = key.toLowerCase();
  if (command && normalized === 's' && !shiftKey) {
    return 'save';
  }
  if (textEditing) {
    return null;
  }
  if (command && normalized === 's' && shiftKey) {
    return 'save-as';
  }
  if (command && normalized === 'z') {
    return shiftKey ? 'redo' : 'undo';
  }
  if (command && normalized === 'y' && !shiftKey) {
    return 'redo';
  }
  if (command && !shiftKey && normalized === 'c') {
    return 'copy-selection';
  }
  if (command && !shiftKey && normalized === 'v') {
    return 'paste-selection';
  }
  if (command && !shiftKey && normalized === 'd') {
    return 'duplicate-selection';
  }
  if (!command && !shiftKey && (key === 'Delete' || key === 'Backspace')) {
    return 'delete-selection';
  }
  return null;
}

export function isTextEditingTarget(target) {
  const tagName = typeof target?.tagName === 'string' ? target.tagName.toLowerCase() : '';
  return (
    target?.isContentEditable === true ||
    tagName === 'input' ||
    tagName === 'textarea' ||
    tagName === 'select'
  );
}
