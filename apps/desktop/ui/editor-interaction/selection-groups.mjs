export function createSelectionGroupIndex(groups = []) {
  if (!Array.isArray(groups)) {
    throw new TypeError('selection groups must be an array');
  }
  const normalized = [];
  const byGroup = new Map();
  const ownerByLeaf = new Map();

  for (const candidate of groups) {
    const groupId = normalizeId(candidate?.groupId, 'groupId');
    if (byGroup.has(groupId)) {
      throw new TypeError(`duplicate selection group: ${groupId}`);
    }
    const leafElementIds = uniqueIds(candidate?.leafElementIds ?? []);
    if (leafElementIds.length === 0) {
      continue;
    }
    for (const leafId of leafElementIds) {
      const existing = ownerByLeaf.get(leafId);
      if (existing && existing !== groupId) {
        throw new TypeError(`rendered element ${leafId} belongs to more than one selection group`);
      }
      ownerByLeaf.set(leafId, groupId);
    }
    const entry = Object.freeze({
      groupId,
      leafElementIds: Object.freeze(leafElementIds),
    });
    normalized.push(entry);
    byGroup.set(groupId, entry);
  }

  return Object.freeze({
    groups: Object.freeze(normalized),
    resolveId(elementId) {
      if (elementId === null || elementId === undefined) {
        return null;
      }
      const id = normalizeId(elementId, 'elementId');
      return ownerByLeaf.get(id) ?? id;
    },
    isGroup(elementId) {
      if (elementId === null || elementId === undefined) {
        return false;
      }
      return byGroup.has(String(elementId));
    },
    renderIds(selectionIds) {
      const output = [];
      const seen = new Set();
      for (const requested of uniqueIds(selectionIds ?? [])) {
        const logicalId = ownerByLeaf.get(requested) ?? requested;
        const group = byGroup.get(logicalId);
        const renderIds = group?.leafElementIds ?? [logicalId];
        for (const renderId of renderIds) {
          if (!seen.has(renderId)) {
            seen.add(renderId);
            output.push(renderId);
          }
        }
      }
      return Object.freeze(output);
    },
    snapIds(selectionIds) {
      const output = [];
      const seen = new Set();
      for (const requested of uniqueIds(selectionIds ?? [])) {
        const logicalId = ownerByLeaf.get(requested) ?? requested;
        for (const id of [logicalId, ...(byGroup.get(logicalId)?.leafElementIds ?? [])]) {
          if (!seen.has(id)) {
            seen.add(id);
            output.push(id);
          }
        }
      }
      return Object.freeze(output);
    },
  });
}

function uniqueIds(values) {
  if (!Array.isArray(values)) {
    throw new TypeError('element IDs must be an array');
  }
  const output = [];
  const seen = new Set();
  for (const value of values) {
    const id = normalizeId(value, 'elementId');
    if (!seen.has(id)) {
      seen.add(id);
      output.push(id);
    }
  }
  return output;
}

function normalizeId(value, label) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new TypeError(`${label} must be a non-empty string`);
  }
  return value;
}
