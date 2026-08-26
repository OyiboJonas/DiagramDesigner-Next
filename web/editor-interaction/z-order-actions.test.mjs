import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createZOrderRequest,
  isZOrderActionEnabled,
} from './z-order-actions.mjs';

test('z-order operations map to the desktop request contract', () => {
  for (const operation of ['bringToFront', 'sendToBack', 'bringForward', 'sendBackward']) {
    assert.deepEqual(createZOrderRequest(operation), { operation });
  }
  assert.throws(() => createZOrderRequest('raise'), /Unsupported z-order operation/);
});

test('z-order actions require a mutable active layer and a selection', () => {
  assert.equal(
    isZOrderActionEnabled({ selectionCount: 1, layerVisible: true, layerLocked: false, busy: false }),
    true,
  );
  assert.equal(
    isZOrderActionEnabled({
      selectionCount: 1,
      layerVisible: true,
      layerLocked: false,
      busy: false,
      containsGroup: true,
    }),
    true,
  );
  assert.equal(
    isZOrderActionEnabled({ selectionCount: 0, layerVisible: true, layerLocked: false, busy: false }),
    false,
  );
  assert.equal(
    isZOrderActionEnabled({ selectionCount: 2, layerVisible: false, layerLocked: false, busy: false }),
    false,
  );
  assert.equal(
    isZOrderActionEnabled({ selectionCount: 2, layerVisible: true, layerLocked: true, busy: false }),
    false,
  );
  assert.equal(
    isZOrderActionEnabled({ selectionCount: 2, layerVisible: true, layerLocked: false, busy: true }),
    false,
  );
});
