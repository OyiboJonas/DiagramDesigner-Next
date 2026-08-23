export class SnapContractError extends Error {
  constructor(message) {
    super(message);
    this.name = "SnapContractError";
  }
}

export function visualBoundsMm(boundsMm, rotationDeg = 0) {
  const rect = normalizeRect(boundsMm);
  if (!Number.isFinite(rotationDeg)) {
    throw new SnapContractError("rotationDeg must be finite");
  }
  const normalizedRotation = ((rotationDeg % 360) + 360) % 360;
  if (normalizedRotation === 0) {
    return Object.freeze(rect);
  }

  const radians = (normalizedRotation * Math.PI) / 180;
  const cos = Math.abs(Math.cos(radians));
  const sin = Math.abs(Math.sin(radians));
  const width = rect.width * cos + rect.height * sin;
  const height = rect.width * sin + rect.height * cos;
  const centerX = rect.x + rect.width / 2;
  const centerY = rect.y + rect.height / 2;
  return Object.freeze({
    x: centerX - width / 2,
    y: centerY - height / 2,
    width,
    height,
  });
}

export function snapMoveDelta({
  deltaMm,
  elementIds,
  elements,
  pageSize,
  gridStepMm = 5,
  thresholdMm = 1,
  gridEnabled = true,
  objectEnabled = true,
} = {}) {
  const rawDelta = validatePoint(deltaMm, "deltaMm");
  const ids = normalizeElementIds(elementIds);
  const normalizedElements = normalizeElements(elements);
  const normalizedPage = validateSize(pageSize);
  validatePositive(gridStepMm, "gridStepMm");
  validateNonNegative(thresholdMm, "thresholdMm");
  if (typeof gridEnabled !== "boolean" || typeof objectEnabled !== "boolean") {
    throw new SnapContractError("gridEnabled and objectEnabled must be booleans");
  }

  if (!gridEnabled && !objectEnabled) {
    return snapResult(rawDelta, null, null);
  }

  const moving = normalizedElements.filter((element) => ids.has(element.elementId));
  if (moving.length === 0) {
    return snapResult(rawDelta, null, null);
  }

  const movingBounds = unionBounds(moving.map((element) => element.visualBoundsMm));
  const movingX = axisAnchors(movingBounds.x, movingBounds.width);
  const movingY = axisAnchors(movingBounds.y, movingBounds.height);

  const targetX = [];
  const targetY = [];
  if (objectEnabled) {
    addTargetAnchors(targetX, axisAnchors(0, normalizedPage.width), "page", null);
    addTargetAnchors(targetY, axisAnchors(0, normalizedPage.height), "page", null);
    for (const element of normalizedElements) {
      if (ids.has(element.elementId)) {
        continue;
      }
      addTargetAnchors(
        targetX,
        axisAnchors(element.visualBoundsMm.x, element.visualBoundsMm.width),
        "object",
        element.elementId,
      );
      addTargetAnchors(
        targetY,
        axisAnchors(element.visualBoundsMm.y, element.visualBoundsMm.height),
        "object",
        element.elementId,
      );
    }
  }

  const objectX = objectEnabled
    ? nearestGuideCorrection(movingX, rawDelta.x, targetX, thresholdMm)
    : null;
  const objectY = objectEnabled
    ? nearestGuideCorrection(movingY, rawDelta.y, targetY, thresholdMm)
    : null;
  const gridX = gridEnabled
    ? nearestGridCorrection(movingX, rawDelta.x, gridStepMm, thresholdMm)
    : null;
  const gridY = gridEnabled
    ? nearestGridCorrection(movingY, rawDelta.y, gridStepMm, thresholdMm)
    : null;

  const snapX = objectX ?? gridX;
  const snapY = objectY ?? gridY;
  return snapResult(
    {
      x: rawDelta.x + (snapX?.correctionMm ?? 0),
      y: rawDelta.y + (snapY?.correctionMm ?? 0),
    },
    snapX?.guide ?? null,
    snapY?.guide ?? null,
  );
}

export function buildRulerTicks(lengthMm, { minorStepMm = 5, majorStepMm = 10 } = {}) {
  validatePositive(lengthMm, "lengthMm");
  validatePositive(minorStepMm, "minorStepMm");
  validatePositive(majorStepMm, "majorStepMm");
  if (majorStepMm < minorStepMm) {
    throw new SnapContractError("majorStepMm must be greater than or equal to minorStepMm");
  }

  const ticks = [];
  const epsilon = Math.max(1e-9, lengthMm * 1e-12);
  const count = Math.floor((lengthMm + epsilon) / minorStepMm);
  for (let index = 0; index <= count; index += 1) {
    const positionMm = index * minorStepMm;
    if (positionMm > lengthMm + epsilon) {
      break;
    }
    ticks.push(rulerTick(positionMm, majorStepMm, epsilon));
  }

  const last = ticks.at(-1)?.positionMm ?? -Infinity;
  if (lengthMm - last > epsilon) {
    ticks.push(
      Object.freeze({
        positionMm: lengthMm,
        major: true,
        label: formatMm(lengthMm),
      }),
    );
  }
  return Object.freeze(ticks);
}

function snapResult(deltaMm, xGuide, yGuide) {
  return Object.freeze({
    deltaMm: Object.freeze({ ...deltaMm }),
    guides: Object.freeze({ x: xGuide, y: yGuide }),
  });
}

function normalizeElements(elements) {
  if (!Array.isArray(elements)) {
    throw new SnapContractError("elements must be an array");
  }
  const normalized = [];
  const seen = new Set();
  for (const element of elements) {
    const elementId = element?.elementId;
    if (typeof elementId !== "string" || elementId.length === 0) {
      throw new SnapContractError("snap element IDs must be non-empty strings");
    }
    if (seen.has(elementId)) {
      throw new SnapContractError(`duplicate snap element ID: ${elementId}`);
    }
    seen.add(elementId);
    normalized.push(
      Object.freeze({
        elementId,
        visualBoundsMm: visualBoundsMm(element.boundsMm, element.rotationDeg ?? 0),
      }),
    );
  }
  return normalized;
}

function normalizeElementIds(elementIds) {
  if (elementIds == null || typeof elementIds[Symbol.iterator] !== "function") {
    throw new SnapContractError("elementIds must be iterable");
  }
  const ids = new Set();
  for (const elementId of elementIds) {
    if (typeof elementId !== "string" || elementId.length === 0) {
      throw new SnapContractError("moving element IDs must be non-empty strings");
    }
    ids.add(elementId);
  }
  if (ids.size === 0) {
    throw new SnapContractError("snapping requires at least one moving element");
  }
  return ids;
}

function validateSize(size) {
  const width = size?.width;
  const height = size?.height;
  validatePositive(width, "pageSize.width");
  validatePositive(height, "pageSize.height");
  return { width, height };
}

function validatePoint(point, name) {
  const x = point?.x;
  const y = point?.y;
  if (!Number.isFinite(x) || !Number.isFinite(y)) {
    throw new SnapContractError(`${name} must contain finite x/y`);
  }
  return { x, y };
}

function normalizeRect(rect) {
  const x = rect?.x;
  const y = rect?.y;
  const width = rect?.width;
  const height = rect?.height;
  if (![x, y, width, height].every(Number.isFinite)) {
    throw new SnapContractError("boundsMm must contain finite x/y/width/height");
  }
  return {
    x: width >= 0 ? x : x + width,
    y: height >= 0 ? y : y + height,
    width: Math.abs(width),
    height: Math.abs(height),
  };
}

function validatePositive(value, name) {
  if (!Number.isFinite(value) || value <= 0) {
    throw new SnapContractError(`${name} must be finite and greater than zero`);
  }
}

function validateNonNegative(value, name) {
  if (!Number.isFinite(value) || value < 0) {
    throw new SnapContractError(`${name} must be finite and non-negative`);
  }
}

function unionBounds(bounds) {
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (const rect of bounds) {
    left = Math.min(left, rect.x);
    top = Math.min(top, rect.y);
    right = Math.max(right, rect.x + rect.width);
    bottom = Math.max(bottom, rect.y + rect.height);
  }
  return { x: left, y: top, width: right - left, height: bottom - top };
}

function axisAnchors(start, length) {
  return [start, start + length / 2, start + length];
}

function addTargetAnchors(targets, positions, source, targetElementId) {
  for (const positionMm of positions) {
    targets.push({ positionMm, source, targetElementId });
  }
}

function nearestGuideCorrection(movingAnchors, deltaMm, targets, thresholdMm) {
  let best = null;
  for (const movingAnchor of movingAnchors) {
    const moved = movingAnchor + deltaMm;
    for (const target of targets) {
      const correctionMm = target.positionMm - moved;
      const distance = Math.abs(correctionMm);
      if (distance > thresholdMm || !isBetter(distance, best)) {
        continue;
      }
      best = {
        distance,
        correctionMm,
        guide: Object.freeze({
          positionMm: target.positionMm,
          source: target.source,
          targetElementId: target.targetElementId,
        }),
      };
    }
  }
  return best;
}

function nearestGridCorrection(movingAnchors, deltaMm, gridStepMm, thresholdMm) {
  let best = null;
  for (const movingAnchor of movingAnchors) {
    const moved = movingAnchor + deltaMm;
    const positionMm = Math.round(moved / gridStepMm) * gridStepMm;
    const correctionMm = positionMm - moved;
    const distance = Math.abs(correctionMm);
    if (distance > thresholdMm || !isBetter(distance, best)) {
      continue;
    }
    best = {
      distance,
      correctionMm,
      guide: Object.freeze({
        positionMm,
        source: "grid",
        targetElementId: null,
      }),
    };
  }
  return best;
}

function isBetter(distance, best) {
  return best === null || distance < best.distance - 1e-9;
}

function rulerTick(positionMm, majorStepMm, epsilon) {
  const ratio = positionMm / majorStepMm;
  const major = Math.abs(ratio - Math.round(ratio)) <= epsilon;
  return Object.freeze({
    positionMm,
    major,
    label: major ? formatMm(positionMm) : null,
  });
}

function formatMm(value) {
  return Number(value.toFixed(6)).toString();
}
