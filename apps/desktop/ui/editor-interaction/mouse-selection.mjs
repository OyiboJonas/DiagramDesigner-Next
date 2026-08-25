export class MouseSelectionError extends Error {
  constructor(message) {
    super(message);
    this.name = "MouseSelectionError";
  }
}

/**
 * Resolve one mouse hit into transient selection and optional move intent.
 *
 * This module owns no DOM/editor state. Modifier clicks only alter selection;
 * normal dragging of an already-selected member moves the whole selected set.
 */
export function resolveMouseSelection({
  currentIds = [],
  hitElementId = null,
  shiftKey = false,
  ctrlKey = false,
  metaKey = false,
} = {}) {
  const current = normalizeIds(currentIds);
  const modified = Boolean(shiftKey || ctrlKey || metaKey);

  if (hitElementId === null) {
    return freezeResult(modified ? current : [], null);
  }
  if (typeof hitElementId !== "string" || hitElementId.length === 0) {
    throw new MouseSelectionError("hitElementId must be null or a non-empty string");
  }

  if (modified) {
    const next = current.includes(hitElementId)
      ? current.filter((id) => id !== hitElementId)
      : [...current, hitElementId];
    return freezeResult(next, null);
  }

  if (current.includes(hitElementId)) {
    return freezeResult(current, current);
  }
  return freezeResult([hitElementId], [hitElementId]);
}

function normalizeIds(ids) {
  if (ids == null || typeof ids[Symbol.iterator] !== "function") {
    throw new MouseSelectionError("currentIds must be iterable");
  }
  const normalized = [];
  const seen = new Set();
  for (const id of ids) {
    if (typeof id !== "string" || id.length === 0) {
      throw new MouseSelectionError("selection IDs must be non-empty strings");
    }
    if (!seen.has(id)) {
      seen.add(id);
      normalized.push(id);
    }
  }
  return normalized;
}

function freezeResult(selectionIds, moveElementIds) {
  return Object.freeze({
    selectionIds: Object.freeze([...selectionIds]),
    moveElementIds:
      moveElementIds === null ? null : Object.freeze([...moveElementIds]),
  });
}
