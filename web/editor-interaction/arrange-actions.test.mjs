import test from 'node:test';
import assert from 'node:assert/strict';

import {
  arrangeMinimumSelection,
  createArrangeRequest,
  isArrangeActionEnabled,
} from './arrange-actions.mjs';

const alignOperations = [
  'alignLeft',
  'alignHorizontalCenter',
  'alignRight',
  'alignTop',
  'alignVerticalCenter',
  'alignBottom',
];
const distributeOperations = ['distributeHorizontal', 'distributeVertical'];

test('all align and distribute operations map to the desktop request contract', () => {
  for (const operation of [...alignOperations, ...distributeOperations]) {
    assert.deepEqual(createArrangeRequest(operation), { operation });
  }
  assert.throws(() => createArrangeRequest('alignMagic'), /Unsupported align\/distribute operation/);
});

test('alignment requires two logical selection items', () => {
  for (const operation of alignOperations) {
    assert.equal(arrangeMinimumSelection(operation), 2);
    assert.equal(isArrangeActionEnabled({
      operation,
      selectionCount: 2,
      layerVisible: true,
      layerLocked: false,
      busy: false,
    }), true);
    assert.equal(isArrangeActionEnabled({
      operation,
      selectionCount: 1,
      layerVisible: true,
      layerLocked: false,
      busy: false,
    }), false);
  }
});

test('distribution requires three logical selection items', () => {
  for (const operation of distributeOperations) {
    assert.equal(arrangeMinimumSelection(operation), 3);
    assert.equal(isArrangeActionEnabled({
      operation,
      selectionCount: 3,
      layerVisible: true,
      layerLocked: false,
      busy: false,
    }), true);
    assert.equal(isArrangeActionEnabled({
      operation,
      selectionCount: 2,
      layerVisible: true,
      layerLocked: false,
      busy: false,
    }), false);
  }
});

test('hidden locked and busy layers disable every arrange operation', () => {
  for (const operation of [...alignOperations, ...distributeOperations]) {
    const selectionCount = arrangeMinimumSelection(operation);
    assert.equal(isArrangeActionEnabled({ operation, selectionCount, layerVisible: false, layerLocked: false }), false);
    assert.equal(isArrangeActionEnabled({ operation, selectionCount, layerVisible: true, layerLocked: true }), false);
    assert.equal(isArrangeActionEnabled({ operation, selectionCount, layerVisible: true, layerLocked: false, busy: true }), false);
  }
});

test('arrange eligibility depends on logical selection count, not group special cases', () => {
  assert.equal(isArrangeActionEnabled({
    operation: 'alignLeft',
    selectionCount: 2,
    layerVisible: true,
    layerLocked: false,
    busy: false,
    containsGroup: true,
  }), true);
});
