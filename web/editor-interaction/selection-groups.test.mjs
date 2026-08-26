import test from 'node:test';
import assert from 'node:assert/strict';

import { createSelectionGroupIndex } from './selection-groups.mjs';

test('group descendants resolve to one logical selection and expand for render and snap', () => {
  const index = createSelectionGroupIndex([
    { groupId: 'group-a', leafElementIds: ['leaf-a', 'leaf-b'] },
  ]);
  assert.equal(index.resolveId('leaf-a'), 'group-a');
  assert.equal(index.resolveId('group-a'), 'group-a');
  assert.equal(index.resolveId('free'), 'free');
  assert.equal(index.isGroup('group-a'), true);
  assert.deepEqual(index.renderIds(['group-a']), ['leaf-a', 'leaf-b']);
  assert.deepEqual(index.renderIds(['leaf-a', 'free']), ['leaf-a', 'leaf-b', 'free']);
  assert.deepEqual(index.snapIds(['group-a']), ['group-a', 'leaf-a', 'leaf-b']);
});

test('selection group index rejects ambiguous descendant ownership', () => {
  assert.throws(
    () =>
      createSelectionGroupIndex([
        { groupId: 'group-a', leafElementIds: ['leaf'] },
        { groupId: 'group-b', leafElementIds: ['leaf'] },
      ]),
    /belongs to more than one selection group/,
  );
});
