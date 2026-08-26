export class ConnectorStyleContractError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ConnectorStyleContractError';
  }
}

export function connectorEnumChoice(value) {
  if (!value || typeof value.kind !== 'string') {
    throw new ConnectorStyleContractError('connector enum value must contain a kind');
  }
  if (value.kind === 'custom') {
    const code = Number(value.code);
    if (!Number.isInteger(code) || code < 0 || code > 65535) {
      throw new ConnectorStyleContractError('custom connector code must be an unsigned 16-bit integer');
    }
    return `custom:${code}`;
  }
  return value.kind;
}

export function connectorEnumRequest(choice) {
  if (typeof choice !== 'string' || choice.length === 0) {
    throw new ConnectorStyleContractError('connector enum choice must be a non-empty string');
  }
  if (choice.startsWith('custom:')) {
    const code = Number(choice.slice('custom:'.length));
    if (!Number.isInteger(code) || code < 0 || code > 65535) {
      throw new ConnectorStyleContractError('custom connector choice contains an invalid code');
    }
    return { kind: 'custom', code };
  }
  return { kind: choice };
}

export function connectorColorHex(color) {
  if (!color) {
    return '#ffffff';
  }
  if (color.kind === 'rgba') {
    const channels = [color.r, color.g, color.b];
    if (!channels.every((channel) => Number.isInteger(channel) && channel >= 0 && channel <= 255)) {
      throw new ConnectorStyleContractError('RGBA connector colour contains an invalid channel');
    }
    return `#${channels.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`;
  }
  if (color.kind === 'system_palette') {
    return '#808080';
  }
  throw new ConnectorStyleContractError(`unsupported connector colour kind: ${String(color.kind)}`);
}

export function connectorRgbaFromHex(value) {
  const hex = String(value ?? '').replace(/^#/, '');
  if (!/^[0-9a-fA-F]{6}$/.test(hex)) {
    throw new ConnectorStyleContractError('connector colour must contain six hexadecimal digits');
  }
  return {
    kind: 'rgba',
    r: Number.parseInt(hex.slice(0, 2), 16),
    g: Number.parseInt(hex.slice(2, 4), 16),
    b: Number.parseInt(hex.slice(4, 6), 16),
    a: 255,
  };
}

export function connectorUsesSecondary({ lineChoice, startChoice, endChoice } = {}) {
  const markerUsesSecondary = (choice) =>
    choice === 'uml_is_a' || choice === 'uml_has_a' || String(choice ?? '').startsWith('custom:');
  return (
    lineChoice === 'outline' ||
    String(lineChoice ?? '').startsWith('custom:') ||
    markerUsesSecondary(startChoice) ||
    markerUsesSecondary(endChoice)
  );
}

export function buildConnectorStyleRequest({
  elementId,
  startChoice,
  endChoice,
  lineChoice,
  secondaryEnabled,
  secondaryHex,
  baselineSecondaryColor = null,
} = {}) {
  if (typeof elementId !== 'string' || elementId.length === 0) {
    throw new ConnectorStyleContractError('connector element id must be present');
  }
  let secondaryColor = null;
  if (secondaryEnabled === true) {
    const displayedBaseline = connectorColorHex(baselineSecondaryColor).toLowerCase();
    secondaryColor =
      baselineSecondaryColor && String(secondaryHex).toLowerCase() === displayedBaseline
        ? baselineSecondaryColor
        : connectorRgbaFromHex(secondaryHex);
  }
  return {
    elementId,
    startMarker: connectorEnumRequest(startChoice),
    endMarker: connectorEnumRequest(endChoice),
    lineStyle: connectorEnumRequest(lineChoice),
    secondaryColor,
  };
}

export function connectorStyleEquals(connector, request) {
  if (!connector || !request) {
    return false;
  }
  return (
    JSON.stringify(connector.startMarker) === JSON.stringify(request.startMarker) &&
    JSON.stringify(connector.endMarker) === JSON.stringify(request.endMarker) &&
    JSON.stringify(connector.lineStyle) === JSON.stringify(request.lineStyle) &&
    JSON.stringify(connector.secondaryColor ?? null) === JSON.stringify(request.secondaryColor ?? null)
  );
}
