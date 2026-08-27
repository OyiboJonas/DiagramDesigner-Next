export class TextLayoutContractError extends Error {
  constructor(message) {
    super(message);
    this.name = 'TextLayoutContractError';
  }
}

function enumKind(value, axis) {
  if (!value || typeof value !== 'object' || typeof value.kind !== 'string') {
    throw new TextLayoutContractError(`${axis} alignment must contain a domain kind`);
  }
  return value.kind;
}

function legacyChoice(value, axis) {
  return `legacy:${axis}:${JSON.stringify(value)}`;
}

export function textHorizontalChoice(value) {
  const kind = enumKind(value, 'horizontal');
  if (kind === 'left' || kind === 'center' || kind === 'right') {
    return kind;
  }
  return legacyChoice(value, 'horizontal');
}

export function textVerticalChoice(value) {
  const kind = enumKind(value, 'vertical');
  if (kind === 'top' || kind === 'center' || kind === 'bottom') {
    return kind;
  }
  return legacyChoice(value, 'vertical');
}

export function textLayoutLegacyLabel(value, axis) {
  const kind = enumKind(value, axis);
  if (axis === 'horizontal') {
    if (kind === 'block_left') return 'Imported · block left';
    if (kind === 'block_right') return 'Imported · block right';
  }
  if (kind === 'legacy_unknown') {
    const raw = Number(value.legacy_value);
    return Number.isInteger(raw)
      ? `Imported · unknown (${raw})`
      : 'Imported · unknown';
  }
  return `Imported · ${kind.replaceAll('_', ' ')}`;
}

export function textLayoutDisplayMargin(value) {
  const margin = Number(value);
  return Number.isFinite(margin) && margin >= 0 ? margin : 0;
}

function standardHorizontal(choice) {
  if (choice === 'left' || choice === 'center' || choice === 'right') {
    return choice;
  }
  throw new TextLayoutContractError('horizontal alignment must be Left, Center or Right');
}

function standardVertical(choice) {
  if (choice === 'top' || choice === 'center' || choice === 'bottom') {
    return choice;
  }
  throw new TextLayoutContractError('vertical alignment must be Top, Center or Bottom');
}

/**
 * Build a partial, restricted text-layout update.
 *
 * Legacy/imported alignment values are represented by opaque choice keys. Keeping
 * that key emits nothing, so the canonical Rust domain value remains untouched.
 * Only a deliberate change to a standard alignment crosses IPC.
 */
export function buildTextLayoutUpdate({
  baseline,
  horizontalChoice,
  verticalChoice,
  marginMm,
} = {}) {
  if (!baseline || typeof baseline !== 'object') {
    throw new TextLayoutContractError('text layout baseline must be present');
  }

  const update = {};
  const baselineHorizontal = textHorizontalChoice(baseline.horizontal);
  const baselineVertical = textVerticalChoice(baseline.vertical);

  if (horizontalChoice !== baselineHorizontal) {
    update.horizontal = standardHorizontal(horizontalChoice);
  }
  if (verticalChoice !== baselineVertical) {
    update.vertical = standardVertical(verticalChoice);
  }

  const margin = Number(marginMm);
  if (!Number.isFinite(margin) || margin < 0) {
    throw new TextLayoutContractError('text inner margin must be a finite non-negative value');
  }
  if (margin !== textLayoutDisplayMargin(baseline.marginMm)) {
    update.marginMm = margin;
  }

  return Object.keys(update).length === 0 ? null : update;
}
