import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildConnectorStyleRequest,
  connectorColorHex,
  connectorEnumChoice,
  connectorEnumRequest,
  connectorStyleEquals,
  connectorUsesSecondary,
} from './connector-style-actions.mjs';

test('standard and custom connector enum values round-trip losslessly', () => {
  assert.equal(connectorEnumChoice({ kind: 'arrow2' }), 'arrow2');
  assert.deepEqual(connectorEnumRequest('arrow2'), { kind: 'arrow2' });
  assert.equal(connectorEnumChoice({ kind: 'custom', code: 513 }), 'custom:513');
  assert.deepEqual(connectorEnumRequest('custom:513'), { kind: 'custom', code: 513 });
});

test('unchanged system-palette secondary colour is preserved exactly', () => {
  const system = { kind: 'system_palette', index: 7 };
  assert.equal(connectorColorHex(system), '#808080');
  const request = buildConnectorStyleRequest({
    elementId: 'connector-1',
    startChoice: 'custom:9',
    endChoice: 'uml_is_a',
    lineChoice: 'outline',
    secondaryEnabled: true,
    secondaryHex: '#808080',
    baselineSecondaryColor: system,
  });
  assert.deepEqual(request.startMarker, { kind: 'custom', code: 9 });
  assert.deepEqual(request.secondaryColor, system);
});

test('deliberate secondary colour edit materializes RGBA while disabled means domain default', () => {
  const edited = buildConnectorStyleRequest({
    elementId: 'connector-1',
    startChoice: 'none',
    endChoice: 'arrow1',
    lineChoice: 'outline',
    secondaryEnabled: true,
    secondaryHex: '#123456',
    baselineSecondaryColor: { kind: 'system_palette', index: 7 },
  });
  assert.deepEqual(edited.secondaryColor, {
    kind: 'rgba', r: 0x12, g: 0x34, b: 0x56, a: 255,
  });

  const defaulted = buildConnectorStyleRequest({
    elementId: 'connector-1',
    startChoice: 'none',
    endChoice: 'none',
    lineChoice: 'solid',
    secondaryEnabled: false,
    secondaryHex: '#ffffff',
    baselineSecondaryColor: null,
  });
  assert.equal(defaulted.secondaryColor, null);
});

test('secondary controls are relevant for outline, UML and unknown custom semantics', () => {
  assert.equal(connectorUsesSecondary({ lineChoice: 'outline', startChoice: 'none', endChoice: 'none' }), true);
  assert.equal(connectorUsesSecondary({ lineChoice: 'solid', startChoice: 'uml_has_a', endChoice: 'none' }), true);
  assert.equal(connectorUsesSecondary({ lineChoice: 'custom:42', startChoice: 'none', endChoice: 'none' }), true);
  assert.equal(connectorUsesSecondary({ lineChoice: 'solid', startChoice: 'none', endChoice: 'arrow1' }), false);
});

test('style equality compares only persisted connector paint state', () => {
  const connector = {
    kind: 'straight',
    start: {},
    end: {},
    startMarker: { kind: 'arrow1' },
    endMarker: { kind: 'arrow2' },
    lineStyle: { kind: 'outline' },
    secondaryColor: null,
  };
  assert.equal(
    connectorStyleEquals(connector, {
      elementId: 'connector-1',
      startMarker: { kind: 'arrow1' },
      endMarker: { kind: 'arrow2' },
      lineStyle: { kind: 'outline' },
      secondaryColor: null,
    }),
    true,
  );
});
