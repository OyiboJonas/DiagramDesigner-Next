export class AppearanceContractError extends Error {
  constructor(message) {
    super(message);
    this.name = 'AppearanceContractError';
  }
}

function colorHex(value, label) {
  const hex = String(value ?? '').toLowerCase();
  if (!/^#[0-9a-f]{6}$/.test(hex)) {
    throw new AppearanceContractError(`${label} must contain six hexadecimal RGB digits`);
  }
  return hex;
}

function gradientAxis(value) {
  if (value !== 'along_x' && value !== 'along_y') {
    throw new AppearanceContractError('gradient axis must be along_x or along_y');
  }
  return value;
}

/**
 * Build the minimal semantic appearance request from inspector state.
 *
 * Colour fields are emitted only when their displayed value changed. That is
 * important for imported system-palette colours: the desktop DTO may display a
 * neutral RGB fallback, while omission keeps the original domain colour intact.
 * Detail fields belonging to a disabled stroke/fill are deliberately omitted so
 * stale disabled controls can never recreate paint that the user just disabled.
 */
export function buildAppearanceRequest({
  elementId,
  baseline,
  strokeEnabled,
  strokeColor,
  strokeWidthMm,
  fillEnabled,
  fillColor,
  fillGradientEnabled,
  fillGradientEndColor,
  fillGradientAxis,
  textColor,
} = {}) {
  if (typeof elementId !== 'string' || elementId.length === 0) {
    throw new AppearanceContractError('appearance element id must be present');
  }
  if (!baseline || typeof baseline !== 'object') {
    throw new AppearanceContractError('appearance baseline must be present');
  }

  const request = { elementId };

  if (baseline.strokeApplicable) {
    const nextEnabled = strokeEnabled === true;
    if (nextEnabled !== baseline.strokeEnabled) {
      request.strokeEnabled = nextEnabled;
    }

    if (nextEnabled) {
      const nextColor = colorHex(strokeColor, 'stroke colour');
      const previousColor = colorHex(baseline.strokeColor, 'baseline stroke colour');
      if (nextColor !== previousColor) {
        request.strokeColor = nextColor;
      }

      const width = Number(strokeWidthMm);
      if (!Number.isFinite(width) || width <= 0) {
        throw new AppearanceContractError('stroke width must be a finite positive value');
      }
      if (width !== Number(baseline.strokeWidthMm)) {
        request.strokeWidthMm = width;
      }
    }
  }

  if (baseline.fillApplicable) {
    const nextFillEnabled = fillEnabled === true;
    if (nextFillEnabled !== baseline.fillEnabled) {
      request.fillEnabled = nextFillEnabled;
    }

    // Fill/gradient detail controls are meaningful only while fill itself is
    // enabled. Disabling fill removes the entire FillStyle in Rust.
    if (nextFillEnabled) {
      const nextFillColor = colorHex(fillColor, 'fill colour');
      const previousFillColor = colorHex(baseline.fillColor, 'baseline fill colour');
      if (nextFillColor !== previousFillColor) {
        request.fillColor = nextFillColor;
      }

      const nextGradientEnabled = fillGradientEnabled === true;
      if (nextGradientEnabled !== baseline.fillGradientEnabled) {
        request.fillGradientEnabled = nextGradientEnabled;
      }

      if (nextGradientEnabled) {
        const nextEndColor = colorHex(fillGradientEndColor, 'gradient end colour');
        const previousEndColor = colorHex(
          baseline.fillGradientEndColor,
          'baseline gradient end colour',
        );
        if (nextEndColor !== previousEndColor) {
          request.fillGradientEndColor = nextEndColor;
        }

        const nextAxis = gradientAxis(fillGradientAxis);
        const previousAxis = gradientAxis(baseline.fillGradientAxis);
        if (nextAxis !== previousAxis) {
          request.fillGradientAxis = nextAxis;
        }
      }
    }
  }

  if (baseline.textColorApplicable) {
    const nextTextColor = colorHex(textColor, 'text colour');
    const previousTextColor = colorHex(baseline.textColor, 'baseline text colour');
    if (nextTextColor !== previousTextColor) {
      request.textColor = nextTextColor;
    }
  }

  return Object.keys(request).length === 1 ? null : request;
}

export function appearanceControlState({ fillEnabled, fillGradientEnabled } = {}) {
  const fill = fillEnabled === true;
  const gradient = fill && fillGradientEnabled === true;
  return Object.freeze({
    fillColorDisabled: !fill,
    gradientToggleDisabled: !fill,
    gradientDetailsDisabled: !gradient,
  });
}
