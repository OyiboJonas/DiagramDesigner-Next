export class TextFormattingContractError extends Error {
  constructor(message) {
    super(message);
    this.name = 'TextFormattingContractError';
  }
}

function cloneStyle(style) {
  if (!style || typeof style !== 'object' || Array.isArray(style)) {
    throw new TextFormattingContractError('editable text must provide a common style baseline');
  }
  return JSON.parse(JSON.stringify(style));
}

function normalizeFontFamily(value) {
  const family = String(value ?? '').trim();
  return family.length === 0 ? null : family;
}

function normalizeFontSize(value) {
  if (value === null || value === undefined || String(value).trim() === '') {
    return null;
  }
  const size = Number(value);
  if (!Number.isInteger(size) || size <= 0 || size > 65535) {
    throw new TextFormattingContractError('font size must be a whole number from 1 to 65535 pt');
  }
  return size;
}

/**
 * Build only the semantic text fields that actually changed.
 *
 * The complete common TextStyle baseline is cloned before the five controls in
 * this first formatting slice are changed. This deliberately preserves imported
 * strikeout/script/overline/symbol-font/colour semantics that are not exposed by
 * the current UI.
 */
export function buildUniformTextUpdate({
  baselineText,
  baselineStyle,
  text,
  fontFamily,
  fontSizePt,
  bold,
  italic,
  underline,
} = {}) {
  const update = {};
  const nextText = String(text ?? '');
  if (nextText !== String(baselineText ?? '')) {
    update.text = nextText;
  }

  const nextStyle = cloneStyle(baselineStyle);
  nextStyle.fontFamily = normalizeFontFamily(fontFamily);
  nextStyle.fontSizePt = normalizeFontSize(fontSizePt);
  nextStyle.bold = bold === true;
  nextStyle.italic = italic === true;
  nextStyle.underline = underline === true;

  if (JSON.stringify(nextStyle) !== JSON.stringify(baselineStyle)) {
    update.textStyle = nextStyle;
  }
  return update;
}
