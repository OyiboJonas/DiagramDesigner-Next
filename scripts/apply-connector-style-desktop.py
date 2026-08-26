#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace(path, old, new):
    p = ROOT / path
    text = p.read_text(encoding='utf-8')
    if old not in text:
        raise SystemExit(f'pattern not found in {path}: {old[:140]!r}')
    p.write_text(text.replace(old, new, 1), encoding='utf-8')


# Tauri command request + DTO.
replace(
    'apps/desktop/src-tauri/src/lib.rs',
    'struct SetConnectorEndpointRequest {\n    element_id: ElementId,\n    side: ConnectorEndpointSideRequest,\n    position_mm: Point,\n    connection: Option<Connection>,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = "camelCase")]\nstruct UpdateElementPropertiesRequest {\n',
    'struct SetConnectorEndpointRequest {\n    element_id: ElementId,\n    side: ConnectorEndpointSideRequest,\n    position_mm: Point,\n    connection: Option<Connection>,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = "camelCase")]\nstruct UpdateConnectorStyleRequest {\n    element_id: ElementId,\n    start_marker: MarkerStyle,\n    end_marker: MarkerStyle,\n    line_style: LineStyle,\n    secondary_color: Option<Color>,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = "camelCase")]\nstruct UpdateElementPropertiesRequest {\n',
)
replace(
    'apps/desktop/src-tauri/src/lib.rs',
    'struct ConnectorPropertiesDto {\n    kind: &\'static str,\n    start: ConnectorEndpointDto,\n    end: ConnectorEndpointDto,\n}\n',
    'struct ConnectorPropertiesDto {\n    kind: &\'static str,\n    start: ConnectorEndpointDto,\n    end: ConnectorEndpointDto,\n    start_marker: MarkerStyle,\n    end_marker: MarkerStyle,\n    line_style: LineStyle,\n    secondary_color: Option<Color>,\n}\n',
)

# Desktop mutation command.
replace(
    'apps/desktop/src-tauri/src/lib.rs',
    '    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn delete_selection(state: State<\'_, DesktopState>) -> Result<ElementEditResultDto, CommandError> {\n',
    '    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn update_connector_style(\n    request: UpdateConnectorStyleRequest,\n    state: State<\'_, DesktopState>,\n) -> Result<ElementEditResultDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    document\n        .session\n        .set_connector_style(\n            request.element_id,\n            request.start_marker,\n            request.end_marker,\n            request.line_style,\n            request.secondary_color,\n        )\n        .map_err(|error| CommandError::new("connector_style_failed", error.to_string()))?;\n    document\n        .session\n        .set_selection([request.element_id])\n        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;\n    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn delete_selection(state: State<\'_, DesktopState>) -> Result<ElementEditResultDto, CommandError> {\n',
)

# Selection DTO now exposes persisted connector paint semantics losslessly.
replace(
    'apps/desktop/src-tauri/src/lib.rs',
    '    Some(ConnectorPropertiesDto {\n        kind,\n        start: connector_endpoint_dto(connector.start),\n        end: connector_endpoint_dto(connector.end),\n    })\n',
    '    Some(ConnectorPropertiesDto {\n        kind,\n        start: connector_endpoint_dto(connector.start),\n        end: connector_endpoint_dto(connector.end),\n        start_marker: connector.start_marker,\n        end_marker: connector.end_marker,\n        line_style: connector.line_style,\n        secondary_color: connector.secondary_color,\n    })\n',
)

# Tauri manifest, ACL permission and capability.
replace(
    'apps/desktop/src-tauri/build.rs',
    '            "set_connector_endpoint",\n            "delete_selection",\n',
    '            "set_connector_endpoint",\n            "update_connector_style",\n            "delete_selection",\n',
)
replace(
    'apps/desktop/src-tauri/permissions/editor.toml',
    '[[permission]]\nidentifier = "allow-set-connector-endpoint"\ndescription = "Allows the main editor window to invoke the set_connector_endpoint application command."\ncommands.allow = ["set_connector_endpoint"]\n\n[[permission]]\nidentifier = "allow-delete-selection"\n',
    '[[permission]]\nidentifier = "allow-set-connector-endpoint"\ndescription = "Allows the main editor window to invoke the set_connector_endpoint application command."\ncommands.allow = ["set_connector_endpoint"]\n\n[[permission]]\nidentifier = "allow-update-connector-style"\ndescription = "Allows the main editor window to invoke the update_connector_style application command."\ncommands.allow = ["update_connector_style"]\n\n[[permission]]\nidentifier = "allow-delete-selection"\n',
)
replace(
    'apps/desktop/src-tauri/capabilities/main-editor.json',
    '    "allow-set-connector-endpoint",\n    "allow-delete-selection",\n',
    '    "allow-set-connector-endpoint",\n    "allow-update-connector-style",\n    "allow-delete-selection",\n',
)
replace(
    'apps/desktop/src-tauri/src/lib.rs',
    '            create_connector,\n            set_connector_endpoint,\n            delete_selection,\n',
    '            create_connector,\n            set_connector_endpoint,\n            update_connector_style,\n            delete_selection,\n',
)

# Frontend helper keeps custom legacy values/system colours round-trippable.
(ROOT / 'apps/desktop/ui/editor-interaction/connector-style-actions.mjs').write_text(r'''export class ConnectorStyleContractError extends Error {
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
''', encoding='utf-8')

(ROOT / 'web/editor-interaction/connector-style-actions.mjs').write_text(
    "export {\n  ConnectorStyleContractError,\n  buildConnectorStyleRequest,\n  connectorColorHex,\n  connectorEnumChoice,\n  connectorEnumRequest,\n  connectorRgbaFromHex,\n  connectorStyleEquals,\n  connectorUsesSecondary,\n} from '../../apps/desktop/ui/editor-interaction/connector-style-actions.mjs';\n",
    encoding='utf-8',
)
(ROOT / 'web/editor-interaction/connector-style-actions.test.mjs').write_text(r'''import test from 'node:test';
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
''', encoding='utf-8')

# Inspector markup.
marker_options = '''\n                  <option value="none">None</option>\n                  <option value="stop">Stop</option>\n                  <option value="circle">Circle</option>\n                  <option value="ball">Ball</option>\n                  <option value="diamond">Diamond</option>\n                  <option value="arrow1">Arrow · open</option>\n                  <option value="arrow2">Arrow · filled</option>\n                  <option value="arrow3">Arrow · concave</option>\n                  <option value="double_arrow">Double arrow</option>\n                  <option value="uml_is_a">UML Is-A</option>\n                  <option value="uml_has_a">UML Has-A</option>\n                  <option value="many">Many / crow's foot</option>'''
line_options = '''\n                  <option value="solid">Solid</option>\n                  <option value="dotted1">Dotted 1</option>\n                  <option value="dotted2">Dotted 2</option>\n                  <option value="short1">Short 1</option>\n                  <option value="short2">Short 2</option>\n                  <option value="long1">Long 1</option>\n                  <option value="long2">Long 2</option>\n                  <option value="dash_dot1">Dash-dot 1</option>\n                  <option value="dash_dot2">Dash-dot 2</option>\n                  <option value="dash_dash">Dash-dash</option>\n                  <option value="outline">Outline</option>'''
connector_form = f'''\n            <form id="connector-style-form" class="appearance-form" hidden>\n              <div class="appearance-heading">\n                <h3>Connector</h3>\n                <span>line & markers</span>\n              </div>\n              <div class="appearance-section">\n                <label class="property-field">Start marker\n                  <select id="connector-start-marker">{marker_options}\n                  </select>\n                </label>\n                <label class="property-field">End marker\n                  <select id="connector-end-marker">{marker_options}\n                  </select>\n                </label>\n                <label class="property-field">Line style\n                  <select id="connector-line-style">{line_options}\n                  </select>\n                </label>\n              </div>\n              <div id="connector-secondary-section" class="appearance-section" hidden>\n                <label class="appearance-toggle"><input id="connector-secondary-enabled" type="checkbox" /> Secondary color</label>\n                <label class="appearance-color-field">Color <input id="connector-secondary-color" type="color" value="#ffffff" /></label>\n                <p id="connector-secondary-note" class="property-note"></p>\n              </div>\n              <button id="apply-connector-style" type="submit">Apply connector style</button>\n            </form>\n'''
replace(
    'apps/desktop/ui/index.html',
    '\n            <form id="selection-appearance-form" class="appearance-form" hidden>\n',
    connector_form + '\n            <form id="selection-appearance-form" class="appearance-form" hidden>\n',
)

# app.js imports.
replace(
    'apps/desktop/ui/app.js',
    "import { isClipboardSelectionActionEnabled, isClipboardShortcutActionEnabled } from './editor-interaction/clipboard-actions.mjs';\n",
    "import { isClipboardSelectionActionEnabled, isClipboardShortcutActionEnabled } from './editor-interaction/clipboard-actions.mjs';\nimport {\n  buildConnectorStyleRequest,\n  connectorColorHex,\n  connectorEnumChoice,\n  connectorStyleEquals,\n  connectorUsesSecondary,\n} from './editor-interaction/connector-style-actions.mjs';\n",
)

# app.js element references.
replace(
    'apps/desktop/ui/app.js',
    "  applyProperties: document.querySelector('#apply-properties'),\n  appearanceForm: document.querySelector('#selection-appearance-form'),\n",
    "  applyProperties: document.querySelector('#apply-properties'),\n  connectorStyleForm: document.querySelector('#connector-style-form'),\n  connectorStartMarker: document.querySelector('#connector-start-marker'),\n  connectorEndMarker: document.querySelector('#connector-end-marker'),\n  connectorLineStyle: document.querySelector('#connector-line-style'),\n  connectorSecondarySection: document.querySelector('#connector-secondary-section'),\n  connectorSecondaryEnabled: document.querySelector('#connector-secondary-enabled'),\n  connectorSecondaryColor: document.querySelector('#connector-secondary-color'),\n  connectorSecondaryNote: document.querySelector('#connector-secondary-note'),\n  applyConnectorStyle: document.querySelector('#apply-connector-style'),\n  appearanceForm: document.querySelector('#selection-appearance-form'),\n",
)
replace(
    'apps/desktop/ui/app.js',
    '  elements.applyProperties,\n  elements.applyAppearance,\n',
    '  elements.applyProperties,\n  elements.applyConnectorStyle,\n  elements.applyAppearance,\n',
)
replace(
    'apps/desktop/ui/app.js',
    'let appearanceBaseline = null;\nlet connectorTool = null;\n',
    'let appearanceBaseline = null;\nlet connectorStyleBaseline = null;\nlet connectorTool = null;\n',
)
replace(
    'apps/desktop/ui/app.js',
    '    elements.applyAppearance.disabled = !primary?.appearance;\n    updateStructureDisabledState();\n',
    '    elements.applyConnectorStyle.disabled = !primary?.connector;\n    elements.applyAppearance.disabled = !primary?.appearance;\n    updateStructureDisabledState();\n',
)

# Selection render hooks.
replace(
    'apps/desktop/ui/app.js',
    '    elements.selectionPropertiesForm.hidden = true;\n    elements.appearanceForm.hidden = true;\n    appearanceBaseline = null;\n    return;\n',
    '    elements.selectionPropertiesForm.hidden = true;\n    elements.connectorStyleForm.hidden = true;\n    elements.appearanceForm.hidden = true;\n    connectorStyleBaseline = null;\n    appearanceBaseline = null;\n    return;\n',
)
replace(
    'apps/desktop/ui/app.js',
    '  elements.propertyTextField.hidden = !hasText;\n  elements.propertyTextNote.hidden = !hasText || primary.textEditable;\n  renderAppearance(primary.appearance);\n',
    '  elements.propertyTextField.hidden = !hasText;\n  elements.propertyTextNote.hidden = !hasText || primary.textEditable;\n  renderConnectorStyle(primary.connector);\n  renderAppearance(primary.appearance);\n',
)

# Connector render/apply functions before basic appearance.
replace(
    'apps/desktop/ui/app.js',
    '\n\nfunction renderAppearance(appearance) {\n',
    r'''

function setConnectorEnumSelect(select, value, label) {
  for (const option of [...select.options]) {
    if (option.dataset.legacyCustom === 'true') {
      option.remove();
    }
  }
  const choice = connectorEnumChoice(value);
  if (choice.startsWith('custom:')) {
    const option = document.createElement('option');
    option.value = choice;
    option.textContent = `${label} · legacy custom (${value.code})`;
    option.dataset.legacyCustom = 'true';
    select.prepend(option);
  }
  select.value = choice;
}

function updateConnectorSecondaryState() {
  const relevant = connectorUsesSecondary({
    lineChoice: elements.connectorLineStyle.value,
    startChoice: elements.connectorStartMarker.value,
    endChoice: elements.connectorEndMarker.value,
  });
  const enabled = elements.connectorSecondaryEnabled.checked;
  elements.connectorSecondarySection.hidden = !relevant && !enabled;
  elements.connectorSecondaryColor.disabled = !enabled;
  if (connectorStyleBaseline?.secondaryColor?.kind === 'system_palette') {
    elements.connectorSecondaryNote.textContent =
      'Imported system colour is preserved exactly until you change this colour picker.';
  } else {
    elements.connectorSecondaryNote.textContent = relevant
      ? 'Used by Outline and UML/custom marker interiors. Disabled uses the domain default.'
      : 'Stored secondary colour is retained even when the current standard style does not use it.';
  }
}

function renderConnectorStyle(connector) {
  elements.connectorStyleForm.hidden = !connector;
  elements.applyConnectorStyle.disabled = !connector || isBusy;
  if (!connector) {
    connectorStyleBaseline = null;
    return;
  }

  connectorStyleBaseline = JSON.parse(JSON.stringify(connector));
  setConnectorEnumSelect(elements.connectorStartMarker, connector.startMarker, 'Start');
  setConnectorEnumSelect(elements.connectorEndMarker, connector.endMarker, 'End');
  setConnectorEnumSelect(elements.connectorLineStyle, connector.lineStyle, 'Line');
  elements.connectorSecondaryEnabled.checked = connector.secondaryColor !== null;
  elements.connectorSecondaryColor.value = connectorColorHex(connector.secondaryColor);
  updateConnectorSecondaryState();
}

async function applyConnectorStyle(event) {
  event.preventDefault();
  const primary = currentSelectionProperties?.primary;
  const baseline = connectorStyleBaseline;
  if (!invoke || !primary?.connector || !baseline) {
    return;
  }

  let request;
  try {
    request = buildConnectorStyleRequest({
      elementId: primary.elementId,
      startChoice: elements.connectorStartMarker.value,
      endChoice: elements.connectorEndMarker.value,
      lineChoice: elements.connectorLineStyle.value,
      secondaryEnabled: elements.connectorSecondaryEnabled.checked,
      secondaryHex: elements.connectorSecondaryColor.value,
      baselineSecondaryColor: baseline.secondaryColor,
    });
  } catch (error) {
    setStatus(String(error?.message ?? error));
    return;
  }
  if (connectorStyleEquals(baseline, request)) {
    setStatus('Connector style unchanged');
    return;
  }

  setBusy(true);
  try {
    const result = await invoke('update_connector_style', { request });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: true });
    const selection = result.selectedElementIds ?? [primary.elementId];
    svgSurface.setSelection(selection);
    keyboardSurface?.syncSelectionState(selection);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus('Connector style updated');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

function renderAppearance(appearance) {
''',
)

# Event wiring.
replace(
    'apps/desktop/ui/app.js',
    "elements.appearanceForm.addEventListener('submit', (event) => {\n  void applyAppearance(event);\n});\nelements.appearanceStrokeEnabled.addEventListener('change', updateAppearanceEnabledState);\n",
    "elements.connectorStyleForm.addEventListener('submit', (event) => {\n  void applyConnectorStyle(event);\n});\nelements.connectorStartMarker.addEventListener('change', updateConnectorSecondaryState);\nelements.connectorEndMarker.addEventListener('change', updateConnectorSecondaryState);\nelements.connectorLineStyle.addEventListener('change', updateConnectorSecondaryState);\nelements.connectorSecondaryEnabled.addEventListener('change', updateConnectorSecondaryState);\n\nelements.appearanceForm.addEventListener('submit', (event) => {\n  void applyAppearance(event);\n});\nelements.appearanceStrokeEnabled.addEventListener('change', updateAppearanceEnabledState);\n",
)
