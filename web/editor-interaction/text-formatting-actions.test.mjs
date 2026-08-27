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
  fontFamily: null,
  fontSizePt: null,
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
  assert.deepEqual(update.textStyle, {
    fontFamily: 'Inter',
    fontSizePt: 14,
    bold: true,
    italic: true,
    underline: true,
  });
});

test('unexposed rich-text style fields never cross the formatting IPC request', () => {
  const richBaseline = {
    ...baselineStyle,
    strikeout: true,
    script: 'superscript',
    overline: true,
    symbolFont: true,
    color: { kind: 'system_palette', index: 9 },
  };
  const update = buildUniformTextUpdate(
    controls({ baselineStyle: richBaseline, bold: true }),
  );
  assert.deepEqual(update.textStyle, {
    fontFamily: null,
    fontSizePt: null,
    bold: true,
    italic: false,
    underline: false,
  });
  for (const key of ['strikeout', 'script', 'overline', 'symbolFont', 'color']) {
    assert.equal(Object.hasOwn(update.textStyle, key), false);
  }
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
