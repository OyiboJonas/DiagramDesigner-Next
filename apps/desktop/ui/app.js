import { createSvgKeyboardSurface } from './svg-keyboard.mjs';
import { createSvgSurface } from './svg-surface.mjs';
import { buildRulerTicks } from './editor-interaction/snapping.mjs';

const invoke = window.__TAURI__?.core?.invoke;

const elements = {
  newDocument: document.querySelector('#new-document'),
  openDocument: document.querySelector('#open-document'),
  saveDocument: document.querySelector('#save-document'),
  undo: document.querySelector('#undo'),
  redo: document.querySelector('#redo'),
  toggleGrid: document.querySelector('#toggle-grid'),
  toggleSnap: document.querySelector('#toggle-snap'),
  addRectangle: document.querySelector('#add-rectangle'),
  addText: document.querySelector('#add-text'),
  deleteSelection: document.querySelector('#delete-selection'),
  rendererBenchmark: document.querySelector('#renderer-benchmark'),
  documentName: document.querySelector('#document-name'),
  documentPath: document.querySelector('#document-path'),
  documentDirty: document.querySelector('#document-dirty'),
  pageCount: document.querySelector('#page-count'),
  historyState: document.querySelector('#history-state'),
  rendererStats: document.querySelector('#renderer-stats'),
  selectionSummary: document.querySelector('#selection-summary'),
  selectionPropertiesForm: document.querySelector('#selection-properties-form'),
  selectionName: document.querySelector('#selection-name'),
  selectionType: document.querySelector('#selection-type'),
  propertyX: document.querySelector('#property-x'),
  propertyY: document.querySelector('#property-y'),
  propertyWidth: document.querySelector('#property-width'),
  propertyHeight: document.querySelector('#property-height'),
  propertyRotation: document.querySelector('#property-rotation'),
  propertyTextField: document.querySelector('#property-text-field'),
  propertyText: document.querySelector('#property-text'),
  propertyTextNote: document.querySelector('#property-text-note'),
  applyProperties: document.querySelector('#apply-properties'),
  rulerX: document.querySelector('#ruler-x'),
  rulerY: document.querySelector('#ruler-y'),
  canvasPage: document.querySelector('#canvas-page'),
  statusMessage: document.querySelector('#status-message'),
  recoveryDialog: document.querySelector('#recovery-dialog'),
  recoveryRestore: document.querySelector('#recovery-restore'),
  recoveryDiscard: document.querySelector('#recovery-discard'),
};

const actionButtons = [
  elements.newDocument,
  elements.openDocument,
  elements.saveDocument,
  elements.undo,
  elements.redo,
  elements.addRectangle,
  elements.addText,
  elements.deleteSelection,
  elements.applyProperties,
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
let keyboardSurface = null;

const svgSurface = createSvgSurface(elements.canvasPage, {
  commitMove: commitSvgMove,
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
  onStatus: setStatus,
});
keyboardSurface.clear();
svgSurface.setInteractionSettings(interactionSettings);
renderInteractionButtons();

function setBusy(busy) {
  for (const button of actionButtons) {
    button.disabled = busy;
  }
  if (!busy) {
    const selectionCount = Number(currentSelectionProperties?.count ?? 0);
    elements.deleteSelection.disabled = selectionCount === 0;
    elements.applyProperties.disabled = !currentSelectionProperties?.primary;
  }
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
  document.title = `${state.dirty ? '● ' : ''}${state.name} — DiagramDesigner Next`;
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
    await refreshSelectionProperties();
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

  const primary = details?.primary ?? null;
  if (!primary) {
    elements.selectionPropertiesForm.hidden = true;
    return;
  }

  elements.selectionPropertiesForm.hidden = false;
  elements.selectionName.textContent = primary.name;
  elements.selectionType.textContent = primary.elementType;
  elements.propertyX.value = String(primary.boundsMm.x);
  elements.propertyY.value = String(primary.boundsMm.y);
  elements.propertyWidth.value = String(primary.boundsMm.width);
  elements.propertyHeight.value = String(primary.boundsMm.height);
  elements.propertyRotation.value = String(primary.rotationDeg);

  const hasText = primary.text !== null && primary.text !== undefined;
  elements.propertyTextField.hidden = !hasText;
  elements.propertyTextNote.hidden = !hasText || primary.textEditable;
  if (hasText) {
    elements.propertyText.value = primary.text;
    elements.propertyText.disabled = !primary.textEditable;
    if (!primary.textEditable) {
      elements.propertyTextNote.textContent =
        'Rich text is shown for reference; this basic editor will not flatten mixed formatting or dynamic fields.';
    }
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
    setStatus(kind === 'text' ? 'Text box created' : 'Rectangle created');
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
    svgSurface.setSelection([]);
    keyboardSurface?.syncSelectionState([]);
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

async function applyElementProperties(event) {
  event.preventDefault();
  const primary = currentSelectionProperties?.primary;
  if (!invoke || !primary) {
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
  void runAction('new_document', undefined, () => 'New document created', {
    syncRecovery: true,
    refreshPresentation: true,
    preserveSelection: false,
  });
});

elements.openDocument.addEventListener('click', () => {
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
    },
  );
});

elements.addRectangle.addEventListener('click', () => {
  void createBasicElement('rectangle');
});

elements.addText.addEventListener('click', () => {
  void createBasicElement('text');
});

elements.deleteSelection.addEventListener('click', () => {
  void deleteCurrentSelection();
});

elements.selectionPropertiesForm.addEventListener('submit', (event) => {
  void applyElementProperties(event);
});

elements.saveDocument.addEventListener('click', () => {
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
});

elements.undo.addEventListener('click', () => {
  void runAction('undo', undefined, () => 'Undo', {
    syncRecovery: true,
    refreshPresentation: true,
  });
});

elements.redo.addEventListener('click', () => {
  void runAction('redo', undefined, () => 'Redo', {
    syncRecovery: true,
    refreshPresentation: true,
  });
});

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

window.diagramDesignerNext = Object.freeze({
  scheduleRecoverySync,
  refreshPresentation,
});

void initializeDesktop();