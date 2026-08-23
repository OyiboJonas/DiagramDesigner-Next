import assert from 'node:assert/strict';
import test from 'node:test';

import {
  createSvgSurface,
  mapClientPointToViewBox,
} from '../../apps/desktop/ui/svg-surface.mjs';
import { createSvgKeyboardSurface } from '../../apps/desktop/ui/svg-keyboard.mjs';

test('Phase-1 production SVG facade exposes the evidence-tested adapters', () => {
  assert.equal(typeof createSvgSurface, 'function');
  assert.equal(typeof createSvgKeyboardSurface, 'function');
  assert.equal(typeof mapClientPointToViewBox, 'function');
});

test('production SVG facade preserves renderer-neutral point mapping', () => {
  assert.deepEqual(
    mapClientPointToViewBox(
      { left: 10, top: 20, width: 200, height: 100 },
      { x: 0, y: 0, width: 400, height: 200 },
      { xPx: 110, yPx: 70 },
    ),
    { x: 200, y: 100 },
  );
});
