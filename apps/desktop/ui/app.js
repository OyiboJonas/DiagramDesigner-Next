import { createSvgKeyboardSurface } from './svg-keyboard.mjs';
import { createSvgSurface } from './svg-surface.mjs';
import { buildRulerTicks } from './editor-interaction/snapping.mjs';
import { isTextEditingTarget, resolveApplicationShortcut } from './editor-interaction/app-shortcuts.mjs';
import { createZOrderRequest, isZOrderActionEnabled } from './editor-interaction/z-order-actions.mjs';
import { isGroupActionEnabled, isUngroupActionEnabled } from './editor-interaction/group-actions.mjs';
import { isClipboardSelectionActionEnabled, isClipboardShortcutActionEnabled } from './editor-interaction/clipboard-actions.mjs';

const invoke = window.__TAURI__?.core?.invoke;

const elements = {
  newDocument: document.querySelector('#new-document'),
  openDocument: document.querySelector('#open-document'),
  saveDocument: document.querySelector('#save-document'),
  undo: document.querySelector('#undo'),
  redo: document.querySelector('#redo'),
  copySelection: document.querySelector('#copy-selection'),
  pasteSelection: document.querySelector('#paste-selection'),
  duplicateSelection: document.querySelector('#duplicate-selection'),
  sendToBack: document.querySelector('#send-to-back'),
  sendBackward: document.querySelector('#send-backward'),
  bringForward: document.querySelector('#bring-forward'),
  bringToFront: document.querySelector('#bring-to-front'),
  groupSelection: document.querySelector('#group-selection'),
  ungroupSelection: document.querySelector('#ungroup-selection'),
  toggleGrid: document.querySelector('#toggle-grid'),
  toggleSnap: document.querySelector('#toggle-snap'),
  addRectangle: document.querySelector('#add-rectangle'),
  addEllipse: document.querySelector('#add-ellipse'),
  addText: document.querySelector('#add-text'),
  drawStraightConnector: document.querySelector('#draw-straight-connector'),
  drawOrthogonalConnector: document.querySelector('#draw-orthogonal-connector'),
  deleteSelection: document.querySelector('#delete-selection'),
  rendererBenchmark: document.querySelector('#renderer-benchmark'),
  documentName: document.querySelector('#document-name'),
  documentPath: document.querySelector('#document-path'),
  documentDirty: document.querySelector('#document-dirty'),
  pageCount: document.querySelector('#page-count'),
  historyState: document.querySelector('#history-state'),
  rendererStats: document.querySelector('#renderer-stats'),
  appVersion: document.querySelector('#app-version'),
  statusTech: document.querySelector('#status-tech'),
  pageSelect: document.querySelector('#page-select'),
  addPage: document.querySelector('#add-page'),
  deletePage: document.querySelector('#delete-page'),
  pagePropertiesForm: document.querySelector('#page-properties-form'),
  pageName: document.querySelector('#page-name'),
  pageWidth: document.querySelector('#page-width'),
  pageHeight: document.querySelector('#page-height'),
  applyPageProperties: document.querySelector('#apply-page-properties'),
  layerSelect: document.querySelector('#layer-select'),
  addLayer: document.querySelector('#add-layer'),
  deleteLayer: document.querySelector('#delete-layer'),
  layerPropertiesForm: document.querySelector('#layer-properties-form'),
  layerName: document.querySelector('#layer-name'),
  layerVisible: document.querySelector('#layer-visible'),
  layerLocked: document.querySelector('#layer-locked'),
  layerElementCount: document.querySelector('#layer-element-count'),
  applyLayerProperties: document.querySelector('#apply-layer-properties'),
  selectionSummary: document.querySelector('#selection-summary'),
  selectionPropertiesForm: document.querySelector('#selection-properties-form'),
  selectionName: document.querySelector('#selection-name'),
  selectionType: document.querySelector('#selection-type'),
  propertyX: document.querySelector('#property-x'),
  propertyY: document.querySelector('#property-y'),
  propertyWidth: document.querySelector('#property-width'),
  propertyHeight: document.querySelector('#property-height'),
  propertyRotation: document.querySelector('#property-rotation'),
  propertyGeometryNote: document.querySelector('#property-geometry-note'),
  propertyTextField: document.querySelector('#property-text-field'),
  propertyText: document.querySelector('#property-text'),
  propertyTextNote: document.querySelector('#property-text-note'),
  applyProperties: document.querySelector('#apply-properties'),
  appearanceForm: document.querySelector('#selection-appearance-form'),
  appearanceStrokeSection: document.querySelector('#appearance-stroke-section'),
  appearanceStrokeEnabled: document.querySelector('#appearance-stroke-enabled'),
  appearanceStrokeColor: document.querySelector('#appearance-stroke-color'),
  appearanceStrokeWidth: document.querySelector('#appearance-stroke-width'),
  appearanceFillSection: document.querySelector('#appearance-fill-section'),
  appearanceFillEnabled: document.querySelector('#appearance-fill-enabled'),
  appearanceFillColor: document.querySelector('#appearance-fill-color'),
  appearanceTextColorField: document.querySelector('#appearance-text-color-field'),
  appearanceTextColor: document.querySelector('#appearance-text-color'),
  applyAppearance: document.querySelector('#apply-appearance'),
  rulerX: document.querySelector('#ruler-x'),
  rulerY: document.querySelector('#ruler-y'),
  canvasPage: document.querySelector('#canvas-page'),
  statusMessage: document.querySelector('#status-message'),
  recoveryDialog: document.querySelector('#recovery-dialog'),
  recoveryRestore: document.querySelector('#recovery-restore'),
  recoveryDiscard: document.querySelector('#recovery-discard'),
};

const zOrderButtons = [
  elements.sendToBack,
  elements.sendBackward,
  elements.bringForward,
  elements.bringToFront,
];
const groupingButtons = [elements.groupSelection, elements.ungroupSelection];

const actionButtons = [
  elements.newDocument,
  elements.openDocument,
  elements.saveDocument,
  elements.undo,
  elements.redo,
  elements.copySelection,
  elements.pasteSelection,
  elements.duplicateSelection,
  ...zOrderButtons,
  ...groupingButtons,
  elements.addRectangle,
  elements.addEllipse,
  elements.addText,
  elements.drawStraightConnector,
  elements.drawOrthogonalConnector,
  elements.deleteSelection,
  elements.applyProperties,
  elements.applyAppearance,
  elements.addPage,
  elements.deletePage,
  elements.applyPageProperties,
  elements.addLayer,
  elements.deleteLayer,
  elements.applyLayerProperties,
  elements.rendererBenchmark,
];

const recoveryButtons = [elements.recoveryRestore, elements.recoveryDiscard];
const interactionSettings = {
  snappingEnabled: true,
  gridVisible: true,
  gridStepMm: 5,
  snapThresholdPx: 8,
};
let recoveryTimer = null;
let presentationRequestSequence = 0;
let currentPresentation = null;
let currentSelectionProperties = null;
let currentNavigation = null;
let appearanceBaseline = null;
let connectorTool = null;
let isBusy = false;
let clipboardAvailable = false;
let keyboardSurface = null;

const svgSurface = createSvgSurface(elements.canvasPage, {
  commitMove: commitSvgMove,
  commitTransform: commitSvgTransform,
  commitConnector: commitSvgConnector,
  commitConnectorEndpoint: commitSvgConnectorEndpoint,
  onSelectionChange: (elementIds) => {
    keyboardSurface?.syncSelectionState(elementIds);
    void syncSelection(elementIds);
  },
  onError: (error) => {
    setStatus(formatInvokeError(error));
  },
});
keyboardSurface = createSvgKeyboardSurface(elements.canvasPage, {
  getSelection: () => svgSurface.selectedElementIds,
  setSelection: (elementIds) => svgSurface.setSelection(elementIds),
  resolveElementId: (elementId) => svgSurface.resolveSelectionId(elementId),
  onStatus: setStatus,
});
keyboardSurface.clear();
svgSurface.setInteractionSettings(interactionSettings);
renderInteractionButtons();
renderNavigation(null);

function setBusy(busy) {
  isBusy = busy;
  for (const button of actionButtons) {
    button.disabled = busy;
  }
  elements.pageSelect.disabled = busy;
  elements.layerSelect.disabled = busy;
  updateZOrderActionState();
  updateGroupingActionState();
  if (!busy) {
    const selectionCount = Number(currentSelectionProperties?.count ?? 0);
    const primary = currentSelectionProperties?.primary ?? null;
    elements.deleteSelection.disabled = selectionCount === 0;
    updateClipboardActionState();
    elements.applyProperties.disabled =
      !primary || (primary.geometryEditable === false && primary.textEditable !== true);
    elements.applyAppearance.disabled = !primary?.appearance;
    updateStructureDisabledState();
  }
}

function updateClipboardActionState() {
  const selectionCount = Number(currentSelectionProperties?.count ?? 0);
  const selectionEnabled = isClipboardSelectionActionEnabled({
    selectionCount,
    busy: isBusy,
  });
  elements.copySelection.disabled = !selectionEnabled;
  elements.duplicateSelection.disabled = !selectionEnabled;
  elements.pasteSelection.disabled = isBusy || !clipboardAvailable;
  elements.copySelection.title = 'Copy the current selection (Ctrl/Cmd+C)';
  elements.duplicateSelection.title = 'Duplicate the current selection (Ctrl/Cmd+D)';
}

function activeLayerForZOrder() {
  const pages = currentNavigation?.pages ?? [];
  const activePage = pages.find((page) => page.pageId === currentNavigation?.activePageId) ?? null;
  return activePage?.layers.find((layer) => layer.layerId === currentNavigation?.activeLayerId) ?? null;
}

function updateZOrderActionState() {
  const selectionCount = Number(currentSelectionProperties?.count ?? 0);
  const activeLayer = activeLayerForZOrder();
  const enabled = isZOrderActionEnabled({
    selectionCount,
    layerVisible: activeLayer?.visible === true,
    layerLocked: activeLayer?.locked !== false,
    busy: isBusy,
  });
  const reason = isBusy
    ? 'Finish the current action first'
    : selectionCount === 0
      ? 'Select one or more elements to arrange them'
      : !activeLayer?.visible
        ? 'Show the active layer before arranging elements'
        : activeLayer?.locked
          ? 'Unlock the active layer before arranging elements'
          : 'Arrange the current selection';
  const enabledTitles = [
    'Send the selection behind all other elements',
    'Move the selection one step backward',
    'Move the selection one step forward',
    'Bring the selection in front of all other elements',
  ];
  zOrderButtons.forEach((button, index) => {
    button.disabled = !enabled;
    button.title = enabled ? enabledTitles[index] : reason;
  });
}

function updateGroupingActionState() {
  const selectionCount = Number(currentSelectionProperties?.count ?? 0);
  const canGroup = isGroupActionEnabled({
    canGroup: currentSelectionProperties?.canGroup === true,
    busy: isBusy,
  });
  const canUngroup = isUngroupActionEnabled({
    canUngroup: currentSelectionProperties?.canUngroup === true,
    busy: isBusy,
  });
  elements.groupSelection.disabled = !canGroup;
  elements.ungroupSelection.disabled = !canUngroup;
  elements.groupSelection.title = canGroup
    ? 'Group the selected adjacent top-level elements'
    : isBusy
      ? 'Finish the current action first'
      : selectionCount < 2
        ? 'Select at least two adjacent top-level elements to group them'
        : 'Grouping requires adjacent top-level elements on the visible, unlocked active layer';
  elements.ungroupSelection.title = canUngroup
    ? 'Ungroup the selected structural group'
    : isBusy
      ? 'Finish the current action first'
      : 'Select one top-level group on the visible, unlocked active layer';
}

function setRecoveryBusy(busy) {
  for (const button of recoveryButtons) {
    button.disabled = busy;
  }
}

function setStatus(message) {
  elements.statusMessage.textContent = message;
}

function renderState(state) {
  elements.documentName.textContent = state.name;
  let pathLabel = 'Not saved yet';
  if (state.path && state.sourcePath) {
    pathLabel = `${state.path} · imported from ${state.sourcePath}`;
  } else if (state.path) {
    pathLabel = state.path;
  } else if (state.sourcePath) {
    pathLabel = `Imported from ${state.sourcePath} · save as .ddnx`;
  }
  elements.documentPath.textContent = pathLabel;
  elements.documentPath.title = pathLabel;

  if (state.recovered) {
    elements.documentDirty.textContent = 'Recovered — save required';
  } else if (state.imported && !state.path) {
    elements.documentDirty.textContent = 'Imported copy — save as DDNX';
  } else {
    elements.documentDirty.textContent = state.dirty ? 'Unsaved changes' : 'Saved';
  }

  elements.pageCount.textContent = String(state.pageCount);
  elements.historyState.textContent = String(state.historyState);
  elements.appVersion.textContent = `Alpha · ${state.version}`;
  elements.statusTech.textContent = `Tauri 2 · DDNX · ${state.version}`;
  document.title = `${state.dirty ? '● ' : ''}${state.name} — DiagramDesigner Next Alpha ${state.version}`;
}

function updateStructureDisabledState() {
  const pages = currentNavigation?.pages ?? [];
  const activePage = pages.find((page) => page.pageId === currentNavigation?.activePageId) ?? null;
  const activeLayer =
    activePage?.layers.find((layer) => layer.layerId === currentNavigation?.activeLayerId) ?? null;
  elements.deletePage.disabled = isBusy || pages.length <= 1 || !activePage;
  elements.addLayer.disabled = isBusy || !activePage;
  elements.deleteLayer.disabled =
    isBusy || !activePage || activePage.layers.length <= 1 || !activeLayer || activeLayer.locked;
  elements.deleteLayer.title = activeLayer?.locked
    ? 'Unlock the layer before deleting it'
    : 'Delete the active layer';
  const layerEditable = Boolean(activeLayer?.visible && !activeLayer?.locked);
  elements.addRectangle.disabled = isBusy || !layerEditable;
  elements.addEllipse.disabled = isBusy || !layerEditable;
  elements.addText.disabled = isBusy || !layerEditable;
  elements.drawStraightConnector.disabled = isBusy || !layerEditable;
  elements.drawOrthogonalConnector.disabled = isBusy || !layerEditable;
  if (!layerEditable && connectorTool !== null) {
    setConnectorTool(null, { announce: false, clearSelection: false });
  }
  elements.addRectangle.title = layerEditable
    ? 'Create a rectangle on the active layer'
    : 'Choose a visible, unlocked layer to create elements';
  elements.addEllipse.title = layerEditable
    ? 'Create an ellipse on the active layer'
    : 'Choose a visible, unlocked layer to create elements';
  elements.addText.title = layerEditable
    ? 'Create a text box on the active layer'
    : 'Choose a visible, unlocked layer to create elements';
  elements.drawStraightConnector.title = layerEditable
    ? 'Draw straight connectors'
    : 'Choose a visible, unlocked layer to draw connectors';
  elements.drawOrthogonalConnector.title = layerEditable
    ? 'Draw orthogonal connectors'
    : 'Choose a visible, unlocked layer to draw connectors';
  elements.applyPageProperties.disabled = isBusy || !activePage;
  elements.applyLayerProperties.disabled = isBusy || !activeLayer;
  updateZOrderActionState();
}

function renderNavigation(navigation) {
  currentNavigation = navigation;
  const pages = navigation?.pages ?? [];
  const pageFragment = document.createDocumentFragment();
  for (const page of pages) {
    const option = document.createElement('option');
    option.value = page.pageId;
    option.textContent = page.name;
    pageFragment.append(option);
  }
  elements.pageSelect.replaceChildren(pageFragment);
  if (navigation?.activePageId) {
    elements.pageSelect.value = navigation.activePageId;
  }

  const activePage = pages.find((page) => page.pageId === navigation?.activePageId) ?? null;
  elements.pagePropertiesForm.hidden = !activePage;
  if (activePage) {
    elements.pageName.value = activePage.name;
    elements.pageWidth.value = String(activePage.sizeMm.width);
    elements.pageHeight.value = String(activePage.sizeMm.height);
  }

  const layerFragment = document.createDocumentFragment();
  for (const layer of activePage?.layers ?? []) {
    const option = document.createElement('option');
    option.value = layer.layerId;
    const flags = `${layer.visible ? '' : ' · hidden'}${layer.locked ? ' · locked' : ''}`;
    option.textContent = `${layer.name}${flags}`;
    layerFragment.append(option);
  }
  elements.layerSelect.replaceChildren(layerFragment);
  if (navigation?.activeLayerId) {
    elements.layerSelect.value = navigation.activeLayerId;
  }

  const activeLayer =
    activePage?.layers.find((layer) => layer.layerId === navigation?.activeLayerId) ?? null;
  elements.layerPropertiesForm.hidden = !activeLayer;
  if (activeLayer) {
    elements.layerName.value = activeLayer.name;
    elements.layerVisible.checked = activeLayer.visible;
    elements.layerLocked.checked = activeLayer.locked;
    elements.layerElementCount.textContent = `${activeLayer.elementCount} stored element${activeLayer.elementCount === 1 ? '' : 's'}`;
  } else {
    elements.layerElementCount.textContent = '';
  }
  updateStructureDisabledState();
}

async function refreshNavigation() {
  if (!invoke) {
    return null;
  }
  try {
    const navigation = await invoke('document_navigation');
    renderNavigation(navigation);
    return navigation;
  } catch (error) {
    setStatus(formatInvokeError(error));
    return null;
  }
}

function renderPresentationStats(presentation) {
  if (!presentation) {
    elements.rendererStats.textContent = 'No active page to render.';
    return;
  }
  const diagnostics = Number(presentation.diagnosticCount ?? 0);
  const rendered = Number(presentation.renderedElements ?? 0);
  const skipped = Number(presentation.skippedElements ?? 0);
  const snapState = interactionSettings.snappingEnabled ? 'snap on' : 'snap off';
  const gridState = interactionSettings.gridVisible
    ? `${interactionSettings.gridStepMm} mm grid`
    : 'grid hidden';
  elements.rendererStats.textContent =
    `SVG · ${rendered} rendered · ${skipped} skipped · ${diagnostics} diagnostics · ${gridState} · ${snapState}`;
}

function renderRulers(presentation) {
  elements.rulerX.replaceChildren();
  elements.rulerY.replaceChildren();
  if (!presentation) {
    return;
  }
  renderRulerAxis(elements.rulerX, buildRulerTicks(presentation.widthMm), presentation.widthMm);
  renderRulerAxis(elements.rulerY, buildRulerTicks(presentation.heightMm), presentation.heightMm);
}

function renderRulerAxis(host, ticks, lengthMm) {
  const fragment = document.createDocumentFragment();
  for (const tick of ticks) {
    const marker = document.createElement('span');
    marker.className = tick.major ? 'ruler-tick major' : 'ruler-tick minor';
    marker.style.setProperty('--ddn-ruler-position', `${(tick.positionMm / lengthMm) * 100}%`);
    if (tick.label !== null) {
      marker.dataset.label = tick.label;
    }
    fragment.append(marker);
  }
  host.replaceChildren(fragment);
}

function renderInteractionButtons() {
  elements.toggleGrid.setAttribute('aria-pressed', String(interactionSettings.gridVisible));
  elements.toggleSnap.setAttribute('aria-pressed', String(interactionSettings.snappingEnabled));
  elements.drawStraightConnector.setAttribute('aria-pressed', String(connectorTool === 'straight'));
  elements.drawOrthogonalConnector.setAttribute('aria-pressed', String(connectorTool === 'orthogonal'));
}

function setConnectorTool(kind, { announce = true, clearSelection = true } = {}) {
  if (kind !== null && kind !== 'straight' && kind !== 'orthogonal') {
    throw new TypeError(`Unsupported connector tool: ${String(kind)}`);
  }
  connectorTool = kind;
  svgSurface.setConnectorTool(kind);
  if (kind !== null && clearSelection) {
    clearLocalSelection();
  }
  renderInteractionButtons();
  if (announce) {
    setStatus(
      kind === null
        ? 'Selection tool active'
        : kind === 'straight'
          ? 'Straight connector tool — drag on the page; Escape exits'
          : 'Orthogonal connector tool — drag on the page; Escape exits',
    );
  }
}

function applyInteractionSettings(message) {
  svgSurface.setInteractionSettings(interactionSettings);
  renderInteractionButtons();
  renderPresentationStats(currentPresentation);
  if (message) {
    setStatus(message);
  }
}

function formatInvokeError(error) {
  if (error && typeof error === 'object') {
    const message = typeof error.message === 'string' ? error.message : JSON.stringify(error);
    const committed =
      error.committed === true ? ' The file operation already crossed its commit point.' : '';
    return `${message}${committed}`;
  }
  return String(error ?? 'Unknown desktop error');
}

async function refreshPresentation({ preserveSelection = true } = {}) {
  if (!invoke) {
    return null;
  }

  const requestSequence = ++presentationRequestSequence;
  try {
    // Keep the evidence-tested Tauri command name as an internal compatibility
    // boundary for Phase 1. The production UI consumes it only through the
    // renderer-neutral presentation DTO and the stable SVG facade above.
    const presentation = await invoke('candidate_page_presentation');
    if (requestSequence !== presentationRequestSequence) {
      return null;
    }
    const restoreKeyboardFocus = keyboardSurface?.hasKeyboardFocus === true;
    currentPresentation = presentation;
    svgSurface.setPresentation(presentation, { preserveSelection });
    keyboardSurface?.refresh({ restoreFocus: restoreKeyboardFocus });
    renderPresentationStats(presentation);
    renderRulers(presentation);
    await Promise.all([refreshSelectionProperties(), refreshNavigation()]);
    return presentation;
  } catch (error) {
    if (requestSequence === presentationRequestSequence) {
      currentPresentation = null;
      svgSurface.clear();
      keyboardSurface?.clear();
      renderRulers(null);
      elements.rendererStats.textContent = 'SVG renderer failed.';
      setStatus(formatInvokeError(error));
    }
    return null;
  }
}

function renderSelectionProperties(details) {
  currentSelectionProperties = details;
  const count = Number(details?.count ?? 0);
  elements.selectionSummary.textContent =
    count === 0 ? 'No selection' : count === 1 ? '1 element' : `${count} elements`;
  elements.deleteSelection.disabled = count === 0;
  updateClipboardActionState();
  updateZOrderActionState();
  updateGroupingActionState();

  const primary = details?.primary ?? null;
  elements.applyProperties.disabled =
    !primary || (primary.geometryEditable === false && primary.textEditable !== true);
  if (!primary) {
    svgSurface.setTransformSelection(null);
    svgSurface.setConnectorEndpointSelection(null);
    elements.selectionPropertiesForm.hidden = true;
    elements.appearanceForm.hidden = true;
    appearanceBaseline = null;
    return;
  }

  svgSurface.setTransformSelection(
    primary.geometryEditable === false
      ? null
      : {
          elementId: primary.elementId,
          boundsMm: primary.boundsMm,
          rotationDeg: primary.rotationDeg,
          geometryEditable: true,
        },
  );
  svgSurface.setConnectorEndpointSelection(
    primary.connector
      ? { elementId: primary.elementId, ...primary.connector }
      : null,
  );
  elements.selectionPropertiesForm.hidden = false;
  elements.selectionName.textContent = primary.name;
  elements.selectionType.textContent = primary.elementType;
  elements.propertyX.value = String(primary.boundsMm.x);
  elements.propertyY.value = String(primary.boundsMm.y);
  elements.propertyWidth.value = String(primary.boundsMm.width);
  elements.propertyHeight.value = String(primary.boundsMm.height);
  elements.propertyRotation.value = String(primary.rotationDeg);
  const geometryEditable = primary.geometryEditable !== false;
  for (const input of [
    elements.propertyX,
    elements.propertyY,
    elements.propertyWidth,
    elements.propertyHeight,
    elements.propertyRotation,
  ]) {
    input.disabled = !geometryEditable;
  }
  elements.propertyGeometryNote.hidden = geometryEditable;

  const hasText = primary.text !== null && primary.text !== undefined;
  elements.propertyTextField.hidden = !hasText;
  elements.propertyTextNote.hidden = !hasText || primary.textEditable;
  renderAppearance(primary.appearance);

  if (hasText) {
    elements.propertyText.value = primary.text;
    elements.propertyText.disabled = !primary.textEditable;
    if (!primary.textEditable) {
      elements.propertyTextNote.textContent =
        'Rich text is shown for reference; this basic editor will not flatten mixed formatting or dynamic fields.';
    }
  }
}


function renderAppearance(appearance) {
  const available = Boolean(
    appearance &&
      (appearance.strokeApplicable || appearance.fillApplicable || appearance.textColorApplicable),
  );
  elements.appearanceForm.hidden = !available;
  elements.applyAppearance.disabled = !available || isBusy;
  if (!available) {
    appearanceBaseline = null;
    return;
  }

  elements.appearanceStrokeSection.hidden = !appearance.strokeApplicable;
  elements.appearanceFillSection.hidden = !appearance.fillApplicable;
  elements.appearanceTextColorField.hidden = !appearance.textColorApplicable;
  elements.appearanceStrokeEnabled.checked = appearance.strokeEnabled;
  elements.appearanceStrokeColor.value = appearance.strokeColor;
  elements.appearanceStrokeWidth.value = String(appearance.strokeWidthMm);
  elements.appearanceFillEnabled.checked = appearance.fillEnabled;
  elements.appearanceFillColor.value = appearance.fillColor;
  elements.appearanceTextColor.value = appearance.textColor;
  appearanceBaseline = Object.freeze({ ...appearance });
  updateAppearanceEnabledState();
}

function updateAppearanceEnabledState() {
  elements.appearanceStrokeColor.disabled = !elements.appearanceStrokeEnabled.checked;
  elements.appearanceStrokeWidth.disabled = !elements.appearanceStrokeEnabled.checked;
  elements.appearanceFillColor.disabled = !elements.appearanceFillEnabled.checked;
}

async function applyAppearance(event) {
  event.preventDefault();
  const primary = currentSelectionProperties?.primary;
  const baseline = appearanceBaseline;
  if (!invoke || !primary || !baseline) {
    return;
  }
  const request = { elementId: primary.elementId };
  if (baseline.strokeApplicable) {
    if (elements.appearanceStrokeEnabled.checked !== baseline.strokeEnabled) {
      request.strokeEnabled = elements.appearanceStrokeEnabled.checked;
    }
    if (elements.appearanceStrokeColor.value.toLowerCase() !== baseline.strokeColor.toLowerCase()) {
      request.strokeColor = elements.appearanceStrokeColor.value;
    }
    const width = Number(elements.appearanceStrokeWidth.value);
    if (!Number.isFinite(width) || width <= 0) {
      setStatus('Stroke width must be a finite positive value');
      return;
    }
    if (width !== baseline.strokeWidthMm) {
      request.strokeWidthMm = width;
    }
  }
  if (baseline.fillApplicable) {
    if (elements.appearanceFillEnabled.checked !== baseline.fillEnabled) {
      request.fillEnabled = elements.appearanceFillEnabled.checked;
    }
    if (elements.appearanceFillColor.value.toLowerCase() !== baseline.fillColor.toLowerCase()) {
      request.fillColor = elements.appearanceFillColor.value;
    }
  }
  if (
    baseline.textColorApplicable &&
    elements.appearanceTextColor.value.toLowerCase() !== baseline.textColor.toLowerCase()
  ) {
    request.textColor = elements.appearanceTextColor.value;
  }
  if (Object.keys(request).length === 1) {
    setStatus('Appearance unchanged');
    return;
  }

  setBusy(true);
  try {
    const result = await invoke('update_element_appearance', { request });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: true });
    const selection = result.selectedElementIds ?? [primary.elementId];
    svgSurface.setSelection(selection);
    keyboardSurface?.syncSelectionState(selection);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus('Appearance updated');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function refreshSelectionProperties() {
  if (!invoke) {
    return null;
  }
  try {
    const details = await invoke('selection_properties');
    renderSelectionProperties(details);
    return details;
  } catch (error) {
    setStatus(formatInvokeError(error));
    return null;
  }
}

async function syncSelection(elementIds) {
  if (!invoke) {
    return;
  }
  try {
    await invoke('set_selection', {
      request: { elementIds: [...elementIds] },
    });
    await refreshSelectionProperties();
  } catch (error) {
    setStatus(formatInvokeError(error));
  }
}

function clearLocalSelection() {
  svgSurface.setSelection([]);
  keyboardSurface?.syncSelectionState([]);
}

async function runStructureAction(
  command,
  args,
  message,
  { persistent = true, preserveSelection = false } = {},
) {
  if (!invoke) {
    return;
  }
  setBusy(true);
  try {
    const result = await invoke(command, args);
    if (result?.state) {
      renderState(result.state);
    }
    renderNavigation(result);
    if (!preserveSelection) {
      clearLocalSelection();
    }
    await refreshPresentation({ preserveSelection });
    if (persistent) {
      scheduleRecoverySync(250);
    }
    setStatus(message);
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function createBasicElement(kind) {
  if (!invoke) {
    return;
  }
  setBusy(true);
  try {
    const result = await invoke('create_basic_element', { request: { kind } });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: false });
    svgSurface.setSelection(result.selectedElementIds ?? []);
    keyboardSurface?.syncSelectionState(result.selectedElementIds ?? []);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus(kind === 'text' ? 'Text box created' : kind === 'ellipse' ? 'Ellipse created' : 'Rectangle created');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function deleteCurrentSelection() {
  if (!invoke) {
    return;
  }
  setBusy(true);
  try {
    const result = await invoke('delete_selection');
    renderState(result.state);
    clearLocalSelection();
    await refreshPresentation({ preserveSelection: false });
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus('Selection deleted');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function copyCurrentSelection() {
  if (!invoke) {
    return;
  }
  setBusy(true);
  try {
    const result = await invoke('copy_selection');
    clipboardAvailable = Number(result?.count ?? 0) > 0;
    setStatus(`Copied ${result.count} ${result.count === 1 ? 'element' : 'elements'}`);
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function pasteCurrentSelection() {
  if (!invoke || !clipboardAvailable) {
    return;
  }
  setBusy(true);
  try {
    const result = await invoke('paste_selection');
    renderState(result.state);
    await refreshPresentation({ preserveSelection: false });
    const selection = result.selectedElementIds ?? [];
    svgSurface.setSelection(selection);
    keyboardSurface?.syncSelectionState(selection);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus(`Pasted ${selection.length} ${selection.length === 1 ? 'element' : 'elements'}`);
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function duplicateCurrentSelection() {
  if (!invoke || Number(currentSelectionProperties?.count ?? 0) === 0) {
    return;
  }
  setBusy(true);
  try {
    const result = await invoke('duplicate_selection');
    renderState(result.state);
    await refreshPresentation({ preserveSelection: false });
    const selection = result.selectedElementIds ?? [];
    svgSurface.setSelection(selection);
    keyboardSurface?.syncSelectionState(selection);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus(`Duplicated ${selection.length} ${selection.length === 1 ? 'element' : 'elements'}`);
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function reorderCurrentSelection(operation) {
  if (!invoke || Number(currentSelectionProperties?.count ?? 0) === 0) {
    return;
  }
  const labels = {
    sendToBack: 'Selection sent to back',
    sendBackward: 'Selection moved backward',
    bringForward: 'Selection moved forward',
    bringToFront: 'Selection brought to front',
  };
  setBusy(true);
  try {
    const result = await invoke('reorder_selection', { request: createZOrderRequest(operation) });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: true });
    const selection = result.selectedElementIds ?? svgSurface.selectedElementIds;
    svgSurface.setSelection(selection);
    keyboardSurface?.syncSelectionState(selection);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus(labels[operation]);
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function groupCurrentSelection() {
  if (!invoke || currentSelectionProperties?.canGroup !== true) {
    return;
  }
  setBusy(true);
  try {
    const result = await invoke('group_selection');
    renderState(result.state);
    await refreshPresentation({ preserveSelection: false });
    const selection = result.selectedElementIds ?? [];
    svgSurface.setSelection(selection);
    keyboardSurface?.syncSelectionState(selection);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus('Selection grouped');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function ungroupCurrentSelection() {
  if (!invoke || currentSelectionProperties?.canUngroup !== true) {
    return;
  }
  setBusy(true);
  try {
    const result = await invoke('ungroup_selection');
    renderState(result.state);
    await refreshPresentation({ preserveSelection: false });
    const selection = result.selectedElementIds ?? [];
    svgSurface.setSelection(selection);
    keyboardSurface?.syncSelectionState(selection);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus('Group dissolved');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function applyElementProperties(event) {
  event.preventDefault();
  const primary = currentSelectionProperties?.primary;
  if (!invoke || !primary) {
    return;
  }
  if (primary.geometryEditable === false) {
    setStatus('This element uses a dedicated geometry tool and cannot be resized in the basic inspector');
    return;
  }
  const numbers = {
    x: Number(elements.propertyX.value),
    y: Number(elements.propertyY.value),
    width: Number(elements.propertyWidth.value),
    height: Number(elements.propertyHeight.value),
    rotation: Number(elements.propertyRotation.value),
  };
  if (
    !Object.values(numbers).every(Number.isFinite) ||
    numbers.width <= 0 ||
    numbers.height <= 0
  ) {
    setStatus('Bounds must be finite and width/height must be greater than zero');
    return;
  }

  const request = {
    elementId: primary.elementId,
    boundsMm: {
      x: numbers.x,
      y: numbers.y,
      width: numbers.width,
      height: numbers.height,
    },
    rotationDeg: numbers.rotation,
  };
  if (primary.textEditable) {
    request.text = elements.propertyText.value;
  }

  setBusy(true);
  try {
    const result = await invoke('update_element_properties', { request });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: true });
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus('Element properties updated');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function commitSvgMove(commit) {
  if (!invoke) {
    throw new Error('Tauri runtime not detected');
  }
  if (commit?.kind !== 'move-elements') {
    throw new TypeError('SVG surface emitted an unsupported semantic command');
  }

  const state = await invoke('commit_move_elements', {
    request: {
      elementIds: [...commit.elementIds],
      deltaMm: { ...commit.deltaMm },
    },
  });
  renderState(state);
  await refreshPresentation({ preserveSelection: true });
  scheduleRecoverySync(250);
  setStatus('Move committed');
  return state;
}

async function commitSvgTransform(commit) {
  if (!invoke) {
    throw new Error('Tauri runtime not detected');
  }
  if (commit?.kind !== 'transform-element') {
    throw new TypeError('SVG surface emitted an unsupported transform command');
  }

  const result = await invoke('update_element_properties', {
    request: {
      elementId: commit.elementId,
      boundsMm: { ...commit.boundsMm },
      rotationDeg: commit.rotationDeg,
      text: null,
    },
  });
  renderState(result.state);
  await refreshPresentation({ preserveSelection: true });
  const selection = result.selectedElementIds ?? [commit.elementId];
  svgSurface.setSelection(selection);
  keyboardSurface?.syncSelectionState(selection);
  await refreshSelectionProperties();
  scheduleRecoverySync(250);
  setStatus('Direct transform committed');
  return result.state;
}

async function commitSvgConnector(commit) {
  if (!invoke) {
    throw new Error('Tauri runtime not detected');
  }
  if (commit?.kind !== 'create-connector') {
    throw new TypeError('SVG surface emitted an unsupported connector command');
  }
  setBusy(true);
  try {
    const result = await invoke('create_connector', {
      request: {
        kind: commit.connectorKind,
        startMm: { ...commit.startMm },
        endMm: { ...commit.endMm },
        startConnection: commit.startConnection
          ? { elementId: commit.startConnection.elementId, portId: commit.startConnection.portId }
          : null,
        endConnection: commit.endConnection
          ? { elementId: commit.endConnection.elementId, portId: commit.endConnection.portId }
          : null,
      },
    });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: false });
    svgSurface.setSelection(result.selectedElementIds ?? []);
    keyboardSurface?.syncSelectionState(result.selectedElementIds ?? []);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus(commit.connectorKind === 'straight' ? 'Straight connector created' : 'Orthogonal connector created');
    return result.state;
  } finally {
    setBusy(false);
  }
}

async function commitSvgConnectorEndpoint(commit) {
  if (!invoke) {
    throw new Error('Tauri runtime not detected');
  }
  if (commit?.kind !== 'set-connector-endpoint') {
    throw new TypeError('SVG surface emitted an unsupported connector endpoint command');
  }

  setBusy(true);
  try {
    const result = await invoke('set_connector_endpoint', {
      request: {
        elementId: commit.elementId,
        side: commit.side,
        positionMm: { ...commit.positionMm },
        connection: commit.connection
          ? {
              elementId: commit.connection.elementId,
              portId: commit.connection.portId,
            }
          : null,
      },
    });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: true });
    const selection = result.selectedElementIds ?? [commit.elementId];
    svgSurface.setSelection(selection);
    keyboardSurface?.syncSelectionState(selection);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus(
      commit.connection
        ? `${commit.side === 'start' ? 'Start' : 'End'} endpoint attached to port`
        : `${commit.side === 'start' ? 'Start' : 'End'} endpoint set free`,
    );
    return result.state;
  } finally {
    setBusy(false);
  }
}

async function syncRecovery() {
  if (!invoke) {
    return;
  }

  try {
    const result = await invoke('sync_recovery');
    if (result?.state) {
      renderState(result.state);
    }
  } catch (error) {
    setStatus(`Recovery checkpoint failed: ${formatInvokeError(error)}`);
  }
}

function scheduleRecoverySync(delay = 800) {
  if (!invoke) {
    return;
  }
  if (recoveryTimer !== null) {
    window.clearTimeout(recoveryTimer);
  }
  recoveryTimer = window.setTimeout(() => {
    recoveryTimer = null;
    void syncRecovery();
  }, delay);
}

async function runAction(command, args, successMessage, options = {}) {
  if (!invoke) {
    setStatus('This shell must run inside the Tauri desktop runtime.');
    return;
  }

  setBusy(true);
  try {
    const result = await invoke(command, args);
    const state = result?.state ?? result;
    if (state) {
      renderState(state);
    }
    if (result?.cancelled) {
      setStatus('Cancelled');
      return;
    }
    if (options.resetClipboard === true) {
      clipboardAvailable = false;
      updateClipboardActionState();
    }

    if (options.refreshPresentation === true) {
      await refreshPresentation({ preserveSelection: options.preserveSelection !== false });
    }

    if (successMessage) {
      setStatus(successMessage(result));
    } else {
      setStatus('Ready');
    }

    if (options.syncRecovery === true) {
      scheduleRecoverySync();
    }
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function restoreRecovery() {
  if (!invoke) {
    return;
  }

  setRecoveryBusy(true);
  try {
    const result = await invoke('restore_recovery');
    renderState(result.state);
    clipboardAvailable = false;
    updateClipboardActionState();
    await refreshPresentation({ preserveSelection: false });
    elements.recoveryDialog.close();
    setBusy(false);
    setStatus('Recovery snapshot restored as an unsaved copy');
    scheduleRecoverySync(250);
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setRecoveryBusy(false);
  }
}

async function discardRecovery() {
  if (!invoke) {
    return;
  }

  setRecoveryBusy(true);
  try {
    await invoke('discard_recovery');
    elements.recoveryDialog.close();
    setBusy(false);
    setStatus('Recovery snapshot discarded');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setRecoveryBusy(false);
  }
}

async function initializeDesktop() {
  if (!invoke) {
    setStatus('Static shell preview — Tauri runtime not detected');
    return;
  }

  setBusy(true);
  try {
    const state = await invoke('document_state');
    renderState(state);
    await refreshPresentation({ preserveSelection: false });

    const recovery = await invoke('recovery_status');
    if (recovery.available) {
      setStatus('Recovery snapshot found — choose Restore or Discard');
      elements.recoveryDialog.showModal();
      return;
    }

    setBusy(false);
    setStatus('Ready');
  } catch (error) {
    setBusy(false);
    setStatus(formatInvokeError(error));
  }
}

elements.newDocument.addEventListener('click', () => {
  setConnectorTool(null, { announce: false });
  void runAction('new_document', undefined, () => 'New document created', {
    syncRecovery: true,
    refreshPresentation: true,
    preserveSelection: false,
    resetClipboard: true,
  });
});

elements.openDocument.addEventListener('click', () => {
  setConnectorTool(null, { announce: false });
  void runAction(
    'open_document',
    undefined,
    (result) =>
      result.state?.imported
        ? 'Legacy file imported as an unsaved Next copy'
        : 'Document opened',
    {
      syncRecovery: true,
      refreshPresentation: true,
      preserveSelection: false,
      resetClipboard: true,
    },
  );
});

elements.pageSelect.addEventListener('change', () => {
  const pageId = elements.pageSelect.value;
  if (pageId) {
    void runStructureAction(
      'activate_page',
      { request: { pageId } },
      'Active page changed',
      { persistent: false },
    );
  }
});

elements.layerSelect.addEventListener('change', () => {
  const pageId = currentNavigation?.activePageId;
  const layerId = elements.layerSelect.value;
  if (pageId && layerId) {
    void runStructureAction(
      'activate_layer',
      { request: { pageId, layerId } },
      'Active layer changed',
      { persistent: false },
    );
  }
});

elements.addPage.addEventListener('click', () => {
  void runStructureAction('create_page', undefined, 'Page created');
});

elements.deletePage.addEventListener('click', () => {
  const pageId = currentNavigation?.activePageId;
  if (pageId) {
    void runStructureAction('delete_page', { request: { pageId } }, 'Page deleted');
  }
});

elements.pagePropertiesForm.addEventListener('submit', (event) => {
  event.preventDefault();
  const pageId = currentNavigation?.activePageId;
  const width = Number(elements.pageWidth.value);
  const height = Number(elements.pageHeight.value);
  if (!pageId || !Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    setStatus('Page width and height must be finite positive values');
    return;
  }
  void runStructureAction(
    'update_page_properties',
    {
      request: {
        pageId,
        name: elements.pageName.value,
        sizeMm: { width, height },
      },
    },
    'Page properties updated',
    { preserveSelection: true },
  );
});

elements.addLayer.addEventListener('click', () => {
  const pageId = currentNavigation?.activePageId;
  if (pageId) {
    void runStructureAction('create_layer', { request: { pageId } }, 'Layer created');
  }
});

elements.deleteLayer.addEventListener('click', () => {
  const pageId = currentNavigation?.activePageId;
  const layerId = currentNavigation?.activeLayerId;
  if (pageId && layerId) {
    void runStructureAction('delete_layer', { request: { pageId, layerId } }, 'Layer deleted');
  }
});

elements.layerPropertiesForm.addEventListener('submit', (event) => {
  event.preventDefault();
  const pageId = currentNavigation?.activePageId;
  const layerId = currentNavigation?.activeLayerId;
  if (!pageId || !layerId) {
    return;
  }
  void runStructureAction(
    'update_layer_properties',
    {
      request: {
        pageId,
        layerId,
        name: elements.layerName.value,
        visible: elements.layerVisible.checked,
        locked: elements.layerLocked.checked,
      },
    },
    'Layer properties updated',
    { preserveSelection: elements.layerVisible.checked && !elements.layerLocked.checked },
  );
});

elements.addRectangle.addEventListener('click', () => {
  void createBasicElement('rectangle');
});

elements.addEllipse.addEventListener('click', () => {
  void createBasicElement('ellipse');
});

elements.addText.addEventListener('click', () => {
  void createBasicElement('text');
});

elements.drawStraightConnector.addEventListener('click', () => {
  setConnectorTool(connectorTool === 'straight' ? null : 'straight');
});

elements.drawOrthogonalConnector.addEventListener('click', () => {
  setConnectorTool(connectorTool === 'orthogonal' ? null : 'orthogonal');
});

elements.copySelection.addEventListener('click', () => {
  void copyCurrentSelection();
});

elements.pasteSelection.addEventListener('click', () => {
  void pasteCurrentSelection();
});

elements.duplicateSelection.addEventListener('click', () => {
  void duplicateCurrentSelection();
});

elements.sendToBack.addEventListener('click', () => {
  void reorderCurrentSelection('sendToBack');
});

elements.sendBackward.addEventListener('click', () => {
  void reorderCurrentSelection('sendBackward');
});

elements.bringForward.addEventListener('click', () => {
  void reorderCurrentSelection('bringForward');
});

elements.bringToFront.addEventListener('click', () => {
  void reorderCurrentSelection('bringToFront');
});

elements.groupSelection.addEventListener('click', () => {
  void groupCurrentSelection();
});

elements.ungroupSelection.addEventListener('click', () => {
  void ungroupCurrentSelection();
});

elements.deleteSelection.addEventListener('click', () => {
  void deleteCurrentSelection();
});

elements.selectionPropertiesForm.addEventListener('submit', (event) => {
  void applyElementProperties(event);
});

elements.appearanceForm.addEventListener('submit', (event) => {
  void applyAppearance(event);
});
elements.appearanceStrokeEnabled.addEventListener('change', updateAppearanceEnabledState);
elements.appearanceFillEnabled.addEventListener('change', updateAppearanceEnabledState);


function saveCurrentDocument() {
  void runAction(
    'save_document',
    undefined,
    (result) => {
      const mode = result.commitMode === 'replaced' ? 'replaced atomically' : 'created atomically';
      return result.cleanupWarning
        ? `Saved (${mode}); temporary cleanup warning`
        : `Saved (${mode})`;
    },
    { syncRecovery: true },
  );
}

function undoCurrentDocument() {
  void runAction('undo', undefined, () => 'Undo', {
    syncRecovery: true,
    refreshPresentation: true,
  });
}

function redoCurrentDocument() {
  void runAction('redo', undefined, () => 'Redo', {
    syncRecovery: true,
    refreshPresentation: true,
  });
}

elements.saveDocument.addEventListener('click', saveCurrentDocument);

elements.undo.addEventListener('click', undoCurrentDocument);

elements.redo.addEventListener('click', redoCurrentDocument);

elements.toggleGrid.addEventListener('click', () => {
  interactionSettings.gridVisible = !interactionSettings.gridVisible;
  applyInteractionSettings(interactionSettings.gridVisible ? 'Grid shown' : 'Grid hidden');
});

elements.toggleSnap.addEventListener('click', () => {
  interactionSettings.snappingEnabled = !interactionSettings.snappingEnabled;
  applyInteractionSettings(
    interactionSettings.snappingEnabled ? 'Snapping enabled' : 'Snapping disabled',
  );
});

elements.rendererBenchmark.addEventListener('click', () => {
  void runAction(
    'open_renderer_benchmark',
    undefined,
    () => 'Native ADR-019 benchmark opened in an isolated fullscreen WebView',
  );
});

elements.recoveryRestore.addEventListener('click', () => {
  void restoreRecovery();
});

elements.recoveryDiscard.addEventListener('click', () => {
  void discardRecovery();
});

elements.recoveryDialog.addEventListener('cancel', (event) => {
  // Startup recovery must be an explicit Restore/Discard decision. Escape must not
  // silently discard the only recovery snapshot.
  event.preventDefault();
});

window.addEventListener(
  'keydown',
  (event) => {
    if (elements.recoveryDialog.open) {
      return;
    }

    if (!isBusy) {
      const shortcut = resolveApplicationShortcut(
        {
          key: event.key,
          ctrlKey: event.ctrlKey,
          metaKey: event.metaKey,
          shiftKey: event.shiftKey,
          altKey: event.altKey,
        },
        { textEditing: isTextEditingTarget(event.target) },
      );
      if (shortcut) {
        const selectionCount = Number(currentSelectionProperties?.count ?? 0);
        if (
          (shortcut === 'delete-selection' && selectionCount === 0) ||
          !isClipboardShortcutActionEnabled({ shortcut, selectionCount, clipboardAvailable })
        ) {
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        if (shortcut === 'save') {
          saveCurrentDocument();
        } else if (shortcut === 'undo') {
          undoCurrentDocument();
        } else if (shortcut === 'redo') {
          redoCurrentDocument();
        } else if (shortcut === 'copy-selection') {
          void copyCurrentSelection();
        } else if (shortcut === 'paste-selection') {
          void pasteCurrentSelection();
        } else if (shortcut === 'duplicate-selection') {
          void duplicateCurrentSelection();
        } else if (shortcut === 'delete-selection') {
          void deleteCurrentSelection();
        }
        return;
      }
    }

    if (event.key !== 'Escape') {
      return;
    }
    if (svgSurface.cancelTransformGesture()) {
      setStatus('Transform cancelled');
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (svgSurface.cancelConnectorEndpointGesture()) {
      setStatus('Connector endpoint edit cancelled');
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (connectorTool !== null) {
      setConnectorTool(null);
      event.preventDefault();
      event.stopPropagation();
    }
  },
  true,
);

window.diagramDesignerNext = Object.freeze({
  scheduleRecoverySync,
  refreshPresentation,
  reportCloseCheckpointError(message) {
    setStatus(`Close blocked: recovery checkpoint failed: ${String(message)}`);
  },
});

void initializeDesktop();