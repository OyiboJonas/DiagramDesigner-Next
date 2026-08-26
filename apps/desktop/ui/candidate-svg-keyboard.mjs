import { KeyboardSelectionController } from './editor-interaction/keyboard-selection.mjs';

const ELEMENT_SELECTOR = '[data-element-id]';
const MOVE_OVERLAY_SELECTOR = '[data-ddn-move-overlay]';
const SNAP_GUIDES_SELECTOR = '[data-ddn-snap-guides]';

/**
 * Keyboard/focus adapter for the SVG renderer candidate.
 *
 * DOM/SVG knowledge stays here. Selection semantics are delegated to the
 * renderer-independent KeyboardSelectionController and the existing candidate
 * selection surface, so a future renderer can replace this adapter without
 * changing keyboard semantics or editor history.
 */
export function createCandidateSvgKeyboardSurface(
  host,
  {
    getSelection,
    setSelection,
    resolveElementId = (elementId) => elementId,
    onStatus = () => {},
  } = {},
) {
  if (!host || typeof host.addEventListener !== 'function') {
    throw new TypeError('candidate keyboard host must be an event target');
  }
  if (typeof getSelection !== 'function' || typeof setSelection !== 'function') {
    throw new TypeError('candidate keyboard selection callbacks must be functions');
  }
  if (typeof resolveElementId !== 'function') {
    throw new TypeError('candidate keyboard element resolver must be a function');
  }
  if (typeof onStatus !== 'function') {
    throw new TypeError('candidate keyboard status callback must be a function');
  }

  const controller = new KeyboardSelectionController();
  let currentElements = [];
  let focusWasInside = false;

  const refresh = ({ restoreFocus = false } = {}) => {
    currentElements = listKeyboardElements(host, resolveElementId);
    const selectedIds = normalizeCurrentSelection(getSelection(), currentElements);
    controller.replaceElements(
      currentElements.map((entry) => entry.id),
      { selectedIds },
    );
    applyAccessibilityState(host, currentElements, controller.snapshot());
    if (restoreFocus && controller.snapshot().activeId) {
      focusById(currentElements, controller.snapshot().activeId);
    }
    return controller.snapshot();
  };

  const syncSelectionState = (selectedIds) => {
    const available = new Set(currentElements.map((entry) => entry.id));
    const filtered = [...selectedIds].filter((id) => available.has(id));
    controller.setSelection(filtered);
    applyAccessibilityState(host, currentElements, controller.snapshot());
    return controller.snapshot();
  };

  const listeners = {
    focusin: (event) => {
      focusWasInside = true;
      const rawElementId = event.target?.getAttribute?.('data-element-id');
      const elementId = rawElementId ? resolveElementId(rawElementId) : null;
      if (elementId && currentElements.some((entry) => entry.id === elementId)) {
        controller.activate(elementId);
        applyAccessibilityState(host, currentElements, controller.snapshot());
      }
    },
    focusout: (event) => {
      if (!host.contains?.(event.relatedTarget)) {
        focusWasInside = false;
      }
    },
    keydown: (event) => {
      if (!focusWasInside && event.target !== host) {
        return;
      }
      const intent = controller.handleKey({
        key: event.key,
        ctrlKey: event.ctrlKey,
        metaKey: event.metaKey,
        shiftKey: event.shiftKey,
      });
      if (!intent.handled) {
        return;
      }

      event.preventDefault?.();
      if (intent.focusId) {
        focusById(currentElements, intent.focusId);
      }
      if (intent.selectionIds !== null && intent.selectionIds !== undefined) {
        setSelection([...intent.selectionIds]);
        syncSelectionState(intent.selectionIds);
        onStatus(selectionStatus(intent.selectionIds));
      } else {
        applyAccessibilityState(host, currentElements, controller.snapshot());
      }
    },
  };

  for (const [type, listener] of Object.entries(listeners)) {
    host.addEventListener(type, listener);
  }

  host.setAttribute('role', 'listbox');
  host.setAttribute('aria-multiselectable', 'true');
  host.setAttribute(
    'aria-description',
    'Use Tab to enter or leave the canvas. Use arrow keys, Home and End to navigate diagram elements; Space or Enter selects; Escape clears selection; Control or Command A selects all.',
  );

  return Object.freeze({
    refresh,
    syncSelectionState,
    get hasKeyboardFocus() {
      return focusWasInside;
    },
    clear() {
      currentElements = [];
      controller.replaceElements([], { selectedIds: [] });
      host.setAttribute('tabindex', '0');
      host.setAttribute('aria-label', 'Active diagram page, empty');
    },
    dispose() {
      for (const [type, listener] of Object.entries(listeners)) {
        host.removeEventListener(type, listener);
      }
      currentElements = [];
    },
  });
}

function listKeyboardElements(host, resolveElementId) {
  const entries = [];
  for (const element of host.querySelectorAll?.(ELEMENT_SELECTOR) ?? []) {
    if (element.closest?.(MOVE_OVERLAY_SELECTOR) || element.closest?.(SNAP_GUIDES_SELECTOR)) {
      continue;
    }
    const rawId = element.getAttribute('data-element-id');
    const id = rawId ? resolveElementId(rawId) : null;
    if (!id || entries.some((entry) => entry.id === id)) {
      continue;
    }
    entries.push({ id, element });
  }
  return entries;
}

function normalizeCurrentSelection(selectedIds, entries) {
  const available = new Set(entries.map((entry) => entry.id));
  return [...selectedIds].filter((id) => available.has(id));
}

function applyAccessibilityState(host, entries, snapshot) {
  const selected = new Set(snapshot.selectedIds);
  const activeId = snapshot.activeId;
  host.setAttribute('tabindex', entries.length === 0 ? '0' : '-1');
  host.setAttribute('aria-label', entries.length === 0 ? 'Active diagram page, empty' : 'Active diagram page');

  for (const [index, entry] of entries.entries()) {
    entry.element.setAttribute('role', 'option');
    entry.element.setAttribute('tabindex', entry.id === activeId ? '0' : '-1');
    entry.element.setAttribute('aria-selected', String(selected.has(entry.id)));
    entry.element.setAttribute('aria-label', `Diagram element ${index + 1}`);
  }
}

function focusById(entries, id) {
  const entry = entries.find((candidate) => candidate.id === id);
  if (entry && typeof entry.element.focus === 'function') {
    entry.element.focus({ preventScroll: false });
  }
}

function selectionStatus(ids) {
  if (ids.length === 0) {
    return 'Selection cleared';
  }
  if (ids.length === 1) {
    return '1 diagram element selected';
  }
  return `${ids.length} diagram elements selected`;
}
