import test from 'node:test';
import assert from 'node:assert/strict';

import {
  isTextEditingTarget,
  resolveApplicationShortcut,
} from './app-shortcuts.mjs';

test('save is global while editor undo/delete stays out of text inputs', () => {
  assert.equal(resolveApplicationShortcut({ key: 's', ctrlKey: true }, { textEditing: true }), 'save');
  assert.equal(resolveApplicationShortcut({ key: 's', ctrlKey: true, shiftKey: true }, { textEditing: true }), null);
  assert.equal(resolveApplicationShortcut({ key: 'z', ctrlKey: true }, { textEditing: true }), null);
  assert.equal(resolveApplicationShortcut({ key: 'Delete' }, { textEditing: true }), null);
});

test('save as resolves only outside text editing', () => {
  assert.equal(resolveApplicationShortcut({ key: 's', ctrlKey: true, shiftKey: true }), 'save-as');
  assert.equal(resolveApplicationShortcut({ key: 'S', metaKey: true, shiftKey: true }), 'save-as');
});

test('standard undo and redo variants resolve outside text editing', () => {
  assert.equal(resolveApplicationShortcut({ key: 'z', ctrlKey: true }), 'undo');
  assert.equal(resolveApplicationShortcut({ key: 'Z', metaKey: true }), 'undo');
  assert.equal(resolveApplicationShortcut({ key: 'z', ctrlKey: true, shiftKey: true }), 'redo');
  assert.equal(resolveApplicationShortcut({ key: 'y', ctrlKey: true }), 'redo');
  assert.equal(resolveApplicationShortcut({ key: 'Y', metaKey: true }), 'redo');
});

test('clipboard productivity shortcuts resolve outside text editing', () => {
  assert.equal(resolveApplicationShortcut({ key: 'c', ctrlKey: true }), 'copy-selection');
  assert.equal(resolveApplicationShortcut({ key: 'V', metaKey: true }), 'paste-selection');
  assert.equal(resolveApplicationShortcut({ key: 'd', ctrlKey: true }), 'duplicate-selection');
  assert.equal(resolveApplicationShortcut({ key: 'c', ctrlKey: true }, { textEditing: true }), null);
  assert.equal(resolveApplicationShortcut({ key: 'v', metaKey: true }, { textEditing: true }), null);
  assert.equal(resolveApplicationShortcut({ key: 'd', ctrlKey: true }, { textEditing: true }), null);
});

test('delete shortcuts require no command or shift modifier', () => {
  assert.equal(resolveApplicationShortcut({ key: 'Delete' }), 'delete-selection');
  assert.equal(resolveApplicationShortcut({ key: 'Backspace' }), 'delete-selection');
  assert.equal(resolveApplicationShortcut({ key: 'Delete', shiftKey: true }), null);
  assert.equal(resolveApplicationShortcut({ key: 'Backspace', metaKey: true }), null);
});

test('alt combinations and unrelated shortcuts are ignored', () => {
  assert.equal(resolveApplicationShortcut({ key: 's', ctrlKey: true, altKey: true }), null);
  assert.equal(resolveApplicationShortcut({ key: 'a', ctrlKey: true }), null);
  assert.equal(resolveApplicationShortcut({ key: 'Escape' }), null);
});

test('editable target detection protects native fields and contenteditable', () => {
  assert.equal(isTextEditingTarget({ tagName: 'INPUT' }), true);
  assert.equal(isTextEditingTarget({ tagName: 'textarea' }), true);
  assert.equal(isTextEditingTarget({ tagName: 'SELECT' }), true);
  assert.equal(isTextEditingTarget({ tagName: 'DIV', isContentEditable: true }), true);
  assert.equal(isTextEditingTarget({ tagName: 'svg', isContentEditable: false }), false);
  assert.equal(isTextEditingTarget(null), false);
});
