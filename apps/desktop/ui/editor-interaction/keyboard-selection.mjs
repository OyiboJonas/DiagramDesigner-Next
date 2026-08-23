export class KeyboardSelectionContractError extends Error {
  constructor(message) {
    super(message);
    this.name = 'KeyboardSelectionContractError';
  }
}

const PREVIOUS_KEYS = new Set(['ArrowLeft', 'ArrowUp']);
const NEXT_KEYS = new Set(['ArrowRight', 'ArrowDown']);

/**
 * Renderer-independent keyboard selection/navigation state.
 *
 * This controller owns only transient focus/selection intent. It does not know
 * about DOM/SVG, Tauri IPC, editor history or persistence. Renderers/adapters
 * provide the ordered element IDs and apply the returned focus/selection intent.
 */
export class KeyboardSelectionController {
  #elementIds = [];
  #selectedIds = [];
  #activeId = null;

  replaceElements(elementIds, { selectedIds = this.#selectedIds } = {}) {
    const ids = normalizeIds(elementIds, 'elementIds');
    const selected = normalizeSelection(selectedIds, ids);
    const previousActive = this.#activeId;

    this.#elementIds = ids;
    this.#selectedIds = selected;
    if (previousActive && ids.includes(previousActive)) {
      this.#activeId = previousActive;
    } else if (selected.length > 0) {
      this.#activeId = selected[0];
    } else {
      this.#activeId = ids[0] ?? null;
    }
    return this.snapshot();
  }

  setSelection(selectedIds) {
    this.#selectedIds = normalizeSelection(selectedIds, this.#elementIds);
    if (this.#selectedIds.length > 0 && !this.#activeId) {
      this.#activeId = this.#selectedIds[0];
    }
    return this.snapshot();
  }

  activate(elementId) {
    if (elementId === null && this.#elementIds.length === 0) {
      this.#activeId = null;
      return this.snapshot();
    }
    if (typeof elementId !== 'string' || !this.#elementIds.includes(elementId)) {
      throw new KeyboardSelectionContractError('active element must exist in elementIds');
    }
    this.#activeId = elementId;
    return this.snapshot();
  }

  handleKey({ key, ctrlKey = false, metaKey = false, shiftKey = false } = {}) {
    if (typeof key !== 'string' || key.length === 0) {
      throw new KeyboardSelectionContractError('keyboard key must be a non-empty string');
    }
    if (this.#elementIds.length === 0 || key === 'Tab') {
      return Object.freeze({ handled: false });
    }

    if (PREVIOUS_KEYS.has(key)) {
      return this.#moveActive(-1);
    }
    if (NEXT_KEYS.has(key)) {
      return this.#moveActive(1);
    }
    if (key === 'Home') {
      return this.#focusIndex(0);
    }
    if (key === 'End') {
      return this.#focusIndex(this.#elementIds.length - 1);
    }
    if ((ctrlKey || metaKey) && key.toLowerCase() === 'a') {
      this.#selectedIds = [...this.#elementIds];
      return this.#selectionIntent();
    }
    if (key === 'Escape') {
      this.#selectedIds = [];
      return this.#selectionIntent();
    }
    if (key === 'Enter' || key === ' ') {
      if (!this.#activeId) {
        return Object.freeze({ handled: false });
      }
      if ((ctrlKey || metaKey) && key === ' ') {
        const selected = new Set(this.#selectedIds);
        if (selected.has(this.#activeId)) {
          selected.delete(this.#activeId);
        } else {
          selected.add(this.#activeId);
        }
        this.#selectedIds = this.#elementIds.filter((id) => selected.has(id));
      } else if (shiftKey && this.#selectedIds.length > 0) {
        const anchorIndex = this.#elementIds.indexOf(this.#selectedIds[0]);
        const activeIndex = this.#elementIds.indexOf(this.#activeId);
        const start = Math.min(anchorIndex, activeIndex);
        const end = Math.max(anchorIndex, activeIndex);
        this.#selectedIds = this.#elementIds.slice(start, end + 1);
      } else {
        this.#selectedIds = [this.#activeId];
      }
      return this.#selectionIntent();
    }

    return Object.freeze({ handled: false });
  }

  snapshot() {
    return Object.freeze({
      elementIds: Object.freeze([...this.#elementIds]),
      selectedIds: Object.freeze([...this.#selectedIds]),
      activeId: this.#activeId,
    });
  }

  #moveActive(delta) {
    const currentIndex = this.#activeId ? this.#elementIds.indexOf(this.#activeId) : 0;
    const nextIndex = Math.max(0, Math.min(this.#elementIds.length - 1, currentIndex + delta));
    return this.#focusIndex(nextIndex);
  }

  #focusIndex(index) {
    this.#activeId = this.#elementIds[index] ?? null;
    return Object.freeze({
      handled: true,
      focusId: this.#activeId,
      selectionIds: null,
    });
  }

  #selectionIntent() {
    return Object.freeze({
      handled: true,
      focusId: this.#activeId,
      selectionIds: Object.freeze([...this.#selectedIds]),
    });
  }
}

function normalizeIds(ids, label) {
  if (ids == null || typeof ids[Symbol.iterator] !== 'function') {
    throw new KeyboardSelectionContractError(`${label} must be iterable`);
  }
  const normalized = [];
  const seen = new Set();
  for (const id of ids) {
    if (typeof id !== 'string' || id.length === 0) {
      throw new KeyboardSelectionContractError(`${label} must contain non-empty strings`);
    }
    if (seen.has(id)) {
      throw new KeyboardSelectionContractError(`${label} must not contain duplicate IDs`);
    }
    seen.add(id);
    normalized.push(id);
  }
  return normalized;
}

function normalizeSelection(selectedIds, elementIds) {
  const selected = normalizeIds(selectedIds, 'selectedIds');
  const allowed = new Set(elementIds);
  for (const id of selected) {
    if (!allowed.has(id)) {
      throw new KeyboardSelectionContractError(`selected element does not exist: ${id}`);
    }
  }
  return selected;
}
