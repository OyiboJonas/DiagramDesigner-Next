import test from 'node:test';
import assert from 'node:assert/strict';

import { isGroupActionEnabled, isUngroupActionEnabled } from './group-actions.mjs';

test('group actions follow backend capability and busy state', () => {
  assert.equal(isGroupActionEnabled({ canGroup: true, busy: false }), true);
  assert.equal(isGroupActionEnabled({ canGroup: false, busy: false }), false);
  assert.equal(isGroupActionEnabled({ canGroup: true, busy: true }), false);
  assert.equal(isUngroupActionEnabled({ canUngroup: true, busy: false }), true);
  assert.equal(isUngroupActionEnabled({ canUngroup: false, busy: false }), false);
  assert.equal(isUngroupActionEnabled({ canUngroup: true, busy: true }), false);
});

test('group actions stay disabled without an explicit backend capability', () => {
  assert.equal(isGroupActionEnabled(), false);
  assert.equal(isUngroupActionEnabled(), false);
  assert.equal(isGroupActionEnabled({ canGroup: 1 }), false);
  assert.equal(isUngroupActionEnabled({ canUngroup: 'true' }), false);
});
