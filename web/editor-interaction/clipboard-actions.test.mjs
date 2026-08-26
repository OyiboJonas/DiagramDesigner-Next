import test from 'node:test';
import assert from 'node:assert/strict';

import { isClipboardSelectionActionEnabled } from './clipboard-actions.mjs';

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
