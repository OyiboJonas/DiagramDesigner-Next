import test from 'node:test';
import assert from 'node:assert/strict';

import {
  TextLayoutContractError,
  buildTextLayoutUpdate,
  textHorizontalChoice,
  textLayoutDisplayMargin,
  textLayoutLegacyLabel,
  textVerticalChoice,
} from './text-layout-actions.mjs';

const standardBaseline = Object.freeze({
  horizontal: { kind: 'left' },
  vertical: { kind: 'top' },
  marginMm: 1,
});

test('unchanged standard layout emits no semantic update', () => {
  assert.equal(
    buildTextLayoutUpdate({
      baseline: standardBaseline,
      horizontalChoice: 'left',
      verticalChoice: 'top',
      marginMm: 1,
    }),
    null,
  );
});

test('standard alignment and margin changes emit only changed fields', () => {
  assert.deepEqual(
    buildTextLayoutUpdate({
      baseline: standardBaseline,
      horizontalChoice: 'center',
      verticalChoice: 'bottom',
      marginMm: 2.5,
    }),
    { horizontal: 'center', vertical: 'bottom', marginMm: 2.5 },
  );
});

test('legacy horizontal and vertical choices remain opaque and lossless while unchanged', () => {
  const baseline = {
    horizontal: { kind: 'block_right' },
    vertical: { kind: 'legacy_unknown', legacy_value: -3 },
    marginMm: 1,
  };
  const horizontalChoice = textHorizontalChoice(baseline.horizontal);
  const verticalChoice = textVerticalChoice(baseline.vertical);
  assert.match(horizontalChoice, /^legacy:horizontal:/);
  assert.match(verticalChoice, /^legacy:vertical:/);
  assert.equal(textLayoutLegacyLabel(baseline.horizontal, 'horizontal'), 'Imported · block right');
  assert.equal(textLayoutLegacyLabel(baseline.vertical, 'vertical'), 'Imported · unknown (-3)');
  assert.equal(
    buildTextLayoutUpdate({
      baseline,
      horizontalChoice,
      verticalChoice,
      marginMm: 1,
    }),
    null,
  );
});

test('deliberate change from legacy alignment emits only the chosen standard value', () => {
  const baseline = {
    horizontal: { kind: 'block_left' },
    vertical: { kind: 'legacy_unknown', legacy_value: 8 },
    marginMm: 0.5,
  };
  assert.deepEqual(
    buildTextLayoutUpdate({
      baseline,
      horizontalChoice: 'right',
      verticalChoice: 'center',
      marginMm: 0.5,
    }),
    { horizontal: 'right', vertical: 'center' },
  );
});

test('negative imported margin displays renderer fallback and remains untouched until changed', () => {
  const baseline = {
    horizontal: { kind: 'left' },
    vertical: { kind: 'top' },
    marginMm: -4,
  };
  assert.equal(textLayoutDisplayMargin(baseline.marginMm), 0);
  assert.equal(
    buildTextLayoutUpdate({
      baseline,
      horizontalChoice: 'left',
      verticalChoice: 'top',
      marginMm: 0,
    }),
    null,
  );
  assert.deepEqual(
    buildTextLayoutUpdate({
      baseline,
      horizontalChoice: 'left',
      verticalChoice: 'top',
      marginMm: 1.25,
    }),
    { marginMm: 1.25 },
  );
});

test('invalid user margin and forged legacy output choices fail before IPC', () => {
  for (const invalid of [-1, Number.NaN, Number.POSITIVE_INFINITY, 'abc']) {
    assert.throws(
      () =>
        buildTextLayoutUpdate({
          baseline: standardBaseline,
          horizontalChoice: 'left',
          verticalChoice: 'top',
          marginMm: invalid,
        }),
      TextLayoutContractError,
    );
  }
  assert.throws(
    () =>
      buildTextLayoutUpdate({
        baseline: standardBaseline,
        horizontalChoice: 'legacy:horizontal:forged',
        verticalChoice: 'top',
        marginMm: 1,
      }),
    TextLayoutContractError,
  );
});
