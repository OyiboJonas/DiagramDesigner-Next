import test from 'node:test';
import assert from 'node:assert/strict';

import {
  isClipboardSelectionActionEnabled,
  isClipboardShortcutActionEnabled,
} from './clipboard-actions.mjs';

test('clipboard selection actions allow structural group selections', () => {
  assert.equal(isClipboardSelectionActionEnabled(), false);
  assert.equal(isClipboardSelectionActionEnabled({ selectionCount: 0, busy: false }), false);
  assert.equal(isClipboardSelectionActionEnabled({ selectionCount: 1, busy: true }), false);
  assert.equal(isClipboardSelectionActionEnabled({ selectionCount: 1, busy: false }), true);
  assert.equal(
    isClipboardSelectionActionEnabled({ selectionCount: 1, busy: false, containsGroup: true }),
    true,
  );
});

test('clipboard keyboard actions do not special-case structural groups', () => {
  assert.equal(
    isClipboardShortcutActionEnabled({
      shortcut: 'copy-selection',
      selectionCount: 1,
      containsGroup: true,
    }),
    true,
  );
  assert.equal(
    isClipboardShortcutActionEnabled({
      shortcut: 'duplicate-selection',
      selectionCount: 1,
      containsGroup: true,
    }),
    true,
  );
  assert.equal(
    isClipboardShortcutActionEnabled({ shortcut: 'copy-selection', selectionCount: 0 }),
    false,
  );
  assert.equal(
    isClipboardShortcutActionEnabled({ shortcut: 'paste-selection', clipboardAvailable: false }),
    false,
  );
  assert.equal(
    isClipboardShortcutActionEnabled({ shortcut: 'paste-selection', clipboardAvailable: true }),
    true,
  );
  assert.equal(isClipboardShortcutActionEnabled({ shortcut: 'save' }), true);
});
