import assert from 'node:assert/strict';
import test from 'node:test';

import { createCandidateSvgKeyboardSurface } from '../../apps/desktop/ui/candidate-svg-keyboard.mjs';
import {
  KeyboardSelectionContractError,
  KeyboardSelectionController,
} from './keyboard-selection.mjs';

test('replacement preserves a surviving active element and validated selection', () => {
  const controller = new KeyboardSelectionController();
  controller.replaceElements(['a', 'b', 'c'], { selectedIds: ['b'] });
  controller.activate('c');
  const snapshot = controller.replaceElements(['a', 'c', 'd'], { selectedIds: ['a'] });
  assert.equal(snapshot.activeId, 'c');
  assert.deepEqual(snapshot.selectedIds, ['a']);
});

test('arrow keys plus Home and End move focus without changing selection', () => {
  const controller = new KeyboardSelectionController();
  controller.replaceElements(['a', 'b', 'c'], { selectedIds: ['a'] });

  assert.deepEqual(controller.handleKey({ key: 'ArrowRight' }), {
    handled: true,
    focusId: 'b',
    selectionIds: null,
  });
  assert.deepEqual(controller.snapshot().selectedIds, ['a']);
  assert.equal(controller.handleKey({ key: 'End' }).focusId, 'c');
  assert.equal(controller.handleKey({ key: 'Home' }).focusId, 'a');
  assert.equal(controller.handleKey({ key: 'ArrowLeft' }).focusId, 'a');
});

test('Enter and Space select the active element as one transient selection intent', () => {
  const controller = new KeyboardSelectionController();
  controller.replaceElements(['a', 'b']);
  controller.handleKey({ key: 'ArrowDown' });

  assert.deepEqual(controller.handleKey({ key: 'Enter' }), {
    handled: true,
    focusId: 'b',
    selectionIds: ['b'],
  });
  controller.activate('a');
  assert.deepEqual(controller.handleKey({ key: ' ' }).selectionIds, ['a']);
});

test('Control or Command A selects all available elements', () => {
  for (const modifier of ['ctrlKey', 'metaKey']) {
    const controller = new KeyboardSelectionController();
    controller.replaceElements(['a', 'b', 'c']);
    const result = controller.handleKey({ key: 'a', [modifier]: true });
    assert.deepEqual(result.selectionIds, ['a', 'b', 'c']);
  }
});

test('Control or Command Space toggles the active element in multi-selection', () => {
  const controller = new KeyboardSelectionController();
  controller.replaceElements(['a', 'b', 'c'], { selectedIds: ['a'] });
  controller.activate('b');
  assert.deepEqual(controller.handleKey({ key: ' ', ctrlKey: true }).selectionIds, ['a', 'b']);
  assert.deepEqual(controller.handleKey({ key: ' ', metaKey: true }).selectionIds, ['a']);
});

test('Shift Space selects the ordered range from the selection anchor to active focus', () => {
  const controller = new KeyboardSelectionController();
  controller.replaceElements(['a', 'b', 'c', 'd'], { selectedIds: ['b'] });
  controller.activate('d');
  assert.deepEqual(controller.handleKey({ key: ' ', shiftKey: true }).selectionIds, ['b', 'c', 'd']);
});

test('Escape clears selection while Tab remains owned by browser focus traversal', () => {
  const controller = new KeyboardSelectionController();
  controller.replaceElements(['a', 'b'], { selectedIds: ['a'] });
  assert.deepEqual(controller.handleKey({ key: 'Escape' }).selectionIds, []);
  assert.deepEqual(controller.handleKey({ key: 'Tab' }), { handled: false });
});

test('invalid or duplicate IDs fail before keyboard state can escape the frontend', () => {
  const controller = new KeyboardSelectionController();
  assert.throws(
    () => controller.replaceElements(['a', 'a']),
    KeyboardSelectionContractError,
  );
  controller.replaceElements(['a']);
  assert.throws(() => controller.setSelection(['missing']), KeyboardSelectionContractError);
  assert.throws(() => controller.activate('missing'), KeyboardSelectionContractError);
});

test('candidate keyboard adapter validates its host and callback boundary on import', () => {
  assert.throws(
    () =>
      createCandidateSvgKeyboardSurface(null, {
        getSelection: () => [],
        setSelection: () => {},
      }),
    TypeError,
  );
  assert.throws(
    () =>
      createCandidateSvgKeyboardSurface(
        { addEventListener() {} },
        { getSelection: null, setSelection: () => {} },
      ),
    TypeError,
  );
});
