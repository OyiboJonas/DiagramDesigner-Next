import test from 'node:test';
import assert from 'node:assert/strict';

import {
  AppearanceContractError,
  appearanceControlState,
  buildAppearanceRequest,
} from './appearance-actions.mjs';

const baseline = Object.freeze({
  strokeApplicable: true,
  strokeEnabled: true,
  strokeColor: '#000000',
  strokeWidthMm: 0.25,
  fillApplicable: true,
  fillEnabled: true,
  fillColor: '#ffffff',
  fillGradientEnabled: false,
  fillGradientEndColor: '#ffffff',
  fillGradientAxis: 'along_x',
  textColorApplicable: false,
  textColor: '#000000',
});

function controls(overrides = {}) {
  return {
    elementId: 'element-1',
    baseline,
    strokeEnabled: true,
    strokeColor: '#000000',
    strokeWidthMm: 0.25,
    fillEnabled: true,
    fillColor: '#ffffff',
    fillGradientEnabled: false,
    fillGradientEndColor: '#ffffff',
    fillGradientAxis: 'along_x',
    textColor: '#000000',
    ...overrides,
  };
}

test('unchanged appearance emits no semantic request', () => {
  assert.equal(buildAppearanceRequest(controls()), null);
});

test('enabling a gradient emits only the explicit mode change when defaults are unchanged', () => {
  assert.deepEqual(
    buildAppearanceRequest(controls({ fillGradientEnabled: true })),
    {
      elementId: 'element-1',
      fillGradientEnabled: true,
    },
  );
});

test('gradient end colour and axis are emitted independently of the start fill colour', () => {
  const gradientBaseline = {
    ...baseline,
    fillColor: '#102030',
    fillGradientEnabled: true,
    fillGradientEndColor: '#405060',
    fillGradientAxis: 'along_x',
  };
  assert.deepEqual(
    buildAppearanceRequest(
      controls({
        baseline: gradientBaseline,
        fillColor: '#102030',
        fillGradientEnabled: true,
        fillGradientEndColor: '#abcdef',
        fillGradientAxis: 'along_y',
      }),
    ),
    {
      elementId: 'element-1',
      fillGradientEndColor: '#abcdef',
      fillGradientAxis: 'along_y',
    },
  );
});

test('changing the gradient start colour never implicitly disables an existing gradient', () => {
  const gradientBaseline = {
    ...baseline,
    fillGradientEnabled: true,
    fillGradientEndColor: '#112233',
    fillGradientAxis: 'along_y',
  };
  assert.deepEqual(
    buildAppearanceRequest(
      controls({
        baseline: gradientBaseline,
        fillColor: '#445566',
        fillGradientEnabled: true,
        fillGradientEndColor: '#112233',
        fillGradientAxis: 'along_y',
      }),
    ),
    {
      elementId: 'element-1',
      fillColor: '#445566',
    },
  );
});

test('disabling the gradient preserves fill while disabling fill ignores gradient detail controls', () => {
  const gradientBaseline = {
    ...baseline,
    fillGradientEnabled: true,
    fillGradientEndColor: '#112233',
  };
  assert.deepEqual(
    buildAppearanceRequest(
      controls({
        baseline: gradientBaseline,
        fillGradientEnabled: false,
        fillGradientEndColor: '#112233',
      }),
    ),
    {
      elementId: 'element-1',
      fillGradientEnabled: false,
    },
  );

  assert.deepEqual(
    buildAppearanceRequest(
      controls({
        baseline: gradientBaseline,
        fillEnabled: false,
        fillColor: '#123456',
        fillGradientEnabled: false,
        fillGradientEndColor: '#abcdef',
        fillGradientAxis: 'along_y',
      }),
    ),
    {
      elementId: 'element-1',
      fillEnabled: false,
    },
  );
});

test('disabling stroke ignores stale detail fields instead of recreating stroke', () => {
  assert.deepEqual(
    buildAppearanceRequest(
      controls({
        strokeEnabled: false,
        strokeColor: '#abcdef',
        strokeWidthMm: 0,
      }),
    ),
    {
      elementId: 'element-1',
      strokeEnabled: false,
    },
  );
});

test('unchanged displayed fallback colours remain omitted so imported system colours stay lossless', () => {
  const importedBaseline = {
    ...baseline,
    fillColor: '#808080',
    fillGradientEnabled: true,
    fillGradientEndColor: '#808080',
  };
  assert.equal(
    buildAppearanceRequest(
      controls({
        baseline: importedBaseline,
        fillColor: '#808080',
        fillGradientEnabled: true,
        fillGradientEndColor: '#808080',
      }),
    ),
    null,
  );
});

test('control state disables gradient details unless both fill and gradient are enabled', () => {
  assert.deepEqual(appearanceControlState({ fillEnabled: false, fillGradientEnabled: true }), {
    fillColorDisabled: true,
    gradientToggleDisabled: true,
    gradientDetailsDisabled: true,
  });
  assert.deepEqual(appearanceControlState({ fillEnabled: true, fillGradientEnabled: false }), {
    fillColorDisabled: false,
    gradientToggleDisabled: false,
    gradientDetailsDisabled: true,
  });
  assert.deepEqual(appearanceControlState({ fillEnabled: true, fillGradientEnabled: true }), {
    fillColorDisabled: false,
    gradientToggleDisabled: false,
    gradientDetailsDisabled: false,
  });
});

test('invalid gradient axis and stroke width fail before IPC', () => {
  assert.throws(
    () => buildAppearanceRequest(controls({ fillGradientEnabled: true, fillGradientAxis: 'diagonal' })),
    AppearanceContractError,
  );
  assert.throws(
    () => buildAppearanceRequest(controls({ strokeWidthMm: 0 })),
    AppearanceContractError,
  );
});
