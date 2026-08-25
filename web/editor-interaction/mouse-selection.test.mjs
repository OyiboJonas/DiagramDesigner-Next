import test from 'node:test';
import assert from 'node:assert/strict';

import { resolveMouseSelection } from './mouse-selection.mjs';

test('normal click selects one element and starts moving only that element', () => {
  assert.deepEqual(
    resolveMouseSelection({ currentIds: ['a', 'b'], hitElementId: 'c' }),
    { selectionIds: ['c'], moveElementIds: ['c'] },
  );
});

test('dragging an already-selected member moves the complete selection', () => {
  assert.deepEqual(
    resolveMouseSelection({ currentIds: ['a', 'b'], hitElementId: 'b' }),
    { selectionIds: ['a', 'b'], moveElementIds: ['a', 'b'] },
  );
});

test('shift ctrl and meta clicks toggle selection without starting a move', () => {
  for (const modifier of ['shiftKey', 'ctrlKey', 'metaKey']) {
    assert.deepEqual(
      resolveMouseSelection({
        currentIds: ['a', 'b'],
        hitElementId: 'c',
        [modifier]: true,
      }),
      { selectionIds: ['a', 'b', 'c'], moveElementIds: null },
    );
    assert.deepEqual(
      resolveMouseSelection({
        currentIds: ['a', 'b'],
        hitElementId: 'a',
        [modifier]: true,
      }),
      { selectionIds: ['b'], moveElementIds: null },
    );
  }
});

test('blank canvas clears without modifiers and preserves selection with modifiers', () => {
  assert.deepEqual(resolveMouseSelection({ currentIds: ['a', 'b'] }), {
    selectionIds: [],
    moveElementIds: null,
  });
  assert.deepEqual(resolveMouseSelection({ currentIds: ['a', 'b'], shiftKey: true }), {
    selectionIds: ['a', 'b'],
    moveElementIds: null,
  });
});
