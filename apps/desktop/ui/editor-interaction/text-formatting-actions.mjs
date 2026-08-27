export class TextFormattingContractError extends Error {
  constructor(message) {
    super(message);
    this.name = 'TextFormattingContractError';
  }
}

function requireStyle(style) {
  if (!style || typeof style !== 'object' || Array.isArray(style)) {
    throw new TextFormattingContractError('editable text must provide a common style baseline');
  }
  return style;
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

function exposedStyle(style) {
  const baseline = requireStyle(style);
  return {
    fontFamily: baseline.fontFamily ?? null,
    fontSizePt: baseline.fontSizePt ?? null,
    bold: baseline.bold === true,
    italic: baseline.italic === true,
    underline: baseline.underline === true,
  };
}

/**
 * Build only the semantic text fields that actually changed.
 *
 * The IPC formatting payload contains only the five controls implemented by this
 * slice. Unexposed rich-text semantics never cross back from the WebView and are
 * preserved by the Rust boundary from the canonical common TextStyle baseline.
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

  const previousStyle = exposedStyle(baselineStyle);
  const nextStyle = {
    fontFamily: normalizeFontFamily(fontFamily),
    fontSizePt: normalizeFontSize(fontSizePt),
    bold: bold === true,
    italic: italic === true,
    underline: underline === true,
  };

  if (JSON.stringify(nextStyle) !== JSON.stringify(previousStyle)) {
    update.textStyle = nextStyle;
  }
  return update;
}
