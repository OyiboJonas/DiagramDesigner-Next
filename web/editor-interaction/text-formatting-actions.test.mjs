import test from 'node:test';
import assert from 'node:assert/strict';

import {
  TextFormattingContractError,
  buildUniformTextUpdate,
} from './text-formatting-actions.mjs';

const baselineStyle = Object.freeze({
  bold: false,
  italic: false,
  underline: false,
  strikeout: true,
  script: 'superscript',
  overline: true,
  symbolFont: false,
  fontFamily: null,
  fontSizePt: null,
  color: { kind: 'system_palette', index: 9 },
});

function controls(overrides = {}) {
  return {
    baselineText: 'Alpha',
    baselineStyle,
    text: 'Alpha',
    fontFamily: '',
    fontSizePt: '',
    bold: false,
    italic: false,
    underline: false,
    ...overrides,
  };
}

test('unchanged uniform text emits no semantic text update', () => {
  assert.deepEqual(buildUniformTextUpdate(controls()), {});
});

test('text and exposed formatting can change together', () => {
  const update = buildUniformTextUpdate(
    controls({
      text: 'Beta',
      fontFamily: 'Inter',
      fontSizePt: '14',
      bold: true,
      italic: true,
      underline: true,
    }),
  );
  assert.equal(update.text, 'Beta');
  assert.equal(update.textStyle.fontFamily, 'Inter');
  assert.equal(update.textStyle.fontSizePt, 14);
  assert.equal(update.textStyle.bold, true);
  assert.equal(update.textStyle.italic, true);
  assert.equal(update.textStyle.underline, true);
});

test('unexposed rich-text style fields and imported colour remain untouched', () => {
  const update = buildUniformTextUpdate(controls({ bold: true }));
  assert.deepEqual(update.textStyle, {
    ...baselineStyle,
    bold: true,
  });
  assert.deepEqual(update.textStyle.color, { kind: 'system_palette', index: 9 });
  assert.equal(update.textStyle.strikeout, true);
  assert.equal(update.textStyle.script, 'superscript');
  assert.equal(update.textStyle.overline, true);
});

test('blank family and size retain or restore document-default semantics', () => {
  const explicit = {
    ...baselineStyle,
    fontFamily: 'Arial',
    fontSizePt: 11,
  };
  const update = buildUniformTextUpdate(
    controls({ baselineStyle: explicit, fontFamily: '   ', fontSizePt: '' }),
  );
  assert.equal(update.textStyle.fontFamily, null);
  assert.equal(update.textStyle.fontSizePt, null);
});

test('font family is trimmed and whole positive point sizes are required', () => {
  const update = buildUniformTextUpdate(
    controls({ fontFamily: '  Segoe UI  ', fontSizePt: 12 }),
  );
  assert.equal(update.textStyle.fontFamily, 'Segoe UI');
  assert.equal(update.textStyle.fontSizePt, 12);

  for (const invalid of [0, -1, 10.5, 65536, 'abc']) {
    assert.throws(
      () => buildUniformTextUpdate(controls({ fontSizePt: invalid })),
      TextFormattingContractError,
    );
  }
});

test('missing common style baseline is rejected before IPC', () => {
  assert.throws(
    () => buildUniformTextUpdate(controls({ baselineStyle: null })),
    TextFormattingContractError,
  );
});
