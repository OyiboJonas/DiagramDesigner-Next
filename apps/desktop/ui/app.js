import { createCandidateSvgKeyboardSurface } from './candidate-svg-keyboard.mjs';
import { createCandidateSvgSurface } from './candidate-svg-surface.mjs';
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
  rendererBenchmark: document.querySelector('#renderer-benchmark'),
  documentName: document.querySelector('#document-name'),
  documentPath: document.querySelector('#document-path'),
  documentDirty: document.querySelector('#document-dirty'),
  pageCount: document.querySelector('#page-count'),
  historyState: document.querySelector('#history-state'),
  rendererStats: document.querySelector('#renderer-stats'),
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
let keyboardSurface = null;

const candidateSurface = createCandidateSvgSurface(elements.canvasPage, {
  commitMove: commitCandidateMove,
  onSelectionChange: (elementIds) => {
    keyboardSurface?.syncSelectionState(elementIds);
    syncSelection(elementIds);
  },
  onError: (error) => {
    setStatus(formatInvokeError(error));
  },
});
keyboardSurface = createCandidateSvgKeyboardSurface(elements.canvasPage, {
  getSelection: () => candidateSurface.selectedElementIds,
  setSelection: (elementIds) => candidateSurface.setSelection(elementIds),
  onStatus: setStatus,
});
keyboardSurface.clear();
candidateSurface.setInteractionSettings(interactionSettings);
renderInteractionButtons();

function setBusy(busy) {
  for (const button of actionButtons) {
    button.disabled = busy;
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
  elements.documentPath.textContent = state.path ?? 'Not saved yet';
  elements.documentPath.title = state.path ?? '';

  if (state.recovered) {
    elements.documentDirty.textContent = 'Recovered — save required';
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
    `SVG candidate · ${rendered} rendered · ${skipped} skipped · ${diagnostics} diagnostics · ${gridState} · ${snapState}`;
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
  candidateSurface.setInteractionSettings(interactionSettings);
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
    const presentation = await invoke('candidate_page_presentation');
    if (requestSequence !== presentationRequestSequence) {
      return null;
    }
    const restoreKeyboardFocus = keyboardSurface?.hasKeyboardFocus === true;
    currentPresentation = presentation;
    candidateSurface.setPresentation(presentation, { preserveSelection });
    keyboardSurface?.refresh({ restoreFocus: restoreKeyboardFocus });
    renderPresentationStats(presentation);
    renderRulers(presentation);
    return presentation;
  } catch (error) {
    if (requestSequence === presentationRequestSequence) {
      currentPresentation = null;
      candidateSurface.clear();
      keyboardSurface?.clear();
      renderRulers(null);
      elements.rendererStats.textContent = 'Candidate renderer failed.';
      setStatus(formatInvokeError(error));
    }
    return null;
  }
}

function syncSelection(elementIds) {
  if (!invoke) {
    return;
  }
  void invoke('set_selection', {
    request: { elementIds: [...elementIds] },
  }).catch((error) => {
    setStatus(formatInvokeError(error));
  });
}

async function commitCandidateMove(commit) {
  if (!invoke) {
    throw new Error('Tauri runtime not detected');
  }
  if (commit?.kind !== 'move-elements') {
    throw new TypeError('candidate surface emitted an unsupported semantic command');
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
  void runAction('open_document', undefined, () => 'Document opened', {
    syncRecovery: true,
    refreshPresentation: true,
    preserveSelection: false,
  });
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
