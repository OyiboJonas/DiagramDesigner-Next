from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


candidate_path = Path("apps/desktop/ui/candidate-svg-surface.mjs")
candidate = candidate_path.read_text(encoding="utf-8")

candidate = replace_once(
    candidate,
    '''import {
  MoveGestureController,
  bindMovePointerSurface,
} from "./editor-interaction/move-gesture.mjs";
import { snapMoveDelta } from "./editor-interaction/snapping.mjs";
''',
    '''import {
  MoveGestureController,
  bindMovePointerSurface,
} from "./editor-interaction/move-gesture.mjs";
import { resolveMouseSelection } from "./editor-interaction/mouse-selection.mjs";
import { snapMoveDelta } from "./editor-interaction/snapping.mjs";
import {
  TransformGestureController,
  bindTransformPointerSurface,
} from "./editor-interaction/transform-gesture.mjs";
''',
    "candidate imports",
)

candidate = replace_once(
    candidate,
    '''const SELECTED_ATTRIBUTE = "data-ddn-selected";
const SNAP_GUIDES_ATTRIBUTE = "data-ddn-snap-guides";
''',
    '''const SELECTED_ATTRIBUTE = "data-ddn-selected";
const TRANSFORM_EDITOR_ATTRIBUTE = "data-ddn-transform-editor";
const TRANSFORM_FRAME_ATTRIBUTE = "data-ddn-transform-frame";
const TRANSFORM_HANDLE_ATTRIBUTE = "data-ddn-transform-handle";
const TRANSFORM_ELEMENT_ID_ATTRIBUTE = "data-ddn-transform-element-id";
const TRANSFORM_PREVIEW_ATTRIBUTE = "data-ddn-transform-preview";
const TRANSFORM_SOURCE_ATTRIBUTE = "data-ddn-transform-source";
const TRANSFORM_ROTATE_GUIDE_ATTRIBUTE = "data-ddn-transform-rotate-guide";
const SNAP_GUIDES_ATTRIBUTE = "data-ddn-snap-guides";
''',
    "candidate transform constants",
)

candidate = replace_once(
    candidate,
    '''    commitMove,
    commitConnector = () => {},
    commitConnectorEndpoint = () => {},
''',
    '''    commitMove,
    commitTransform = () => {},
    commitConnector = () => {},
    commitConnectorEndpoint = () => {},
''',
    "candidate constructor callback",
)

candidate = replace_once(
    candidate,
    '''  if (typeof commitMove !== "function") {
    throw new TypeError("commitMove must be a function");
  }
  if (typeof commitConnector !== "function") {
''',
    '''  if (typeof commitMove !== "function") {
    throw new TypeError("commitMove must be a function");
  }
  if (typeof commitTransform !== "function") {
    throw new TypeError("commitTransform must be a function");
  }
  if (typeof commitConnector !== "function") {
''',
    "candidate callback validation",
)

candidate = replace_once(
    candidate,
    '''  let disposePointerBinding = null;
  let connectorController = null;
''',
    '''  let disposePointerBinding = null;
  let transformController = null;
  let transformSelection = null;
  let transformCommitPending = false;
  let connectorController = null;
''',
    "candidate transform state",
)

candidate = replace_once(
    candidate,
    '''    if (
      endpointSelection &&
      (applied.length !== 1 || applied[0] !== endpointSelection.elementId)
    ) {
      endpointSelection = null;
      removeConnectorEndpointEditor(svg);
    }
    if (changed && notify) {
''',
    '''    if (
      endpointSelection &&
      (applied.length !== 1 || applied[0] !== endpointSelection.elementId)
    ) {
      endpointSelection = null;
      removeConnectorEndpointEditor(svg);
    }
    if (
      transformSelection &&
      (applied.length !== 1 || applied[0] !== transformSelection.elementId)
    ) {
      transformSelection = null;
      removeTransformEditor(svg);
    }
    if (changed && notify) {
''',
    "candidate selection transform invalidation",
)

candidate = replace_once(
    candidate,
    '''    removeMoveOverlay(svg);
    if (!preview || !Array.isArray(preview.elementIds) || preview.elementIds.length === 0) {
      removeSnapGuides(svg);
      return;
    }

    const group = document.createElementNS(SVG_NS, "g");
''',
    '''    removeMoveOverlay(svg);
    if (!preview || !Array.isArray(preview.elementIds) || preview.elementIds.length === 0) {
      removeSnapGuides(svg);
      renderTransformEditor(
        svg,
        connectorTool === null ? transformSelection : null,
      );
      return;
    }
    removeTransformEditor(svg);

    const group = document.createElementNS(SVG_NS, "g");
''',
    "candidate move preview transform editor",
)

candidate = replace_once(
    candidate,
    '''    const moveController = new MoveGestureController({
      screenToDocument,
      transformDelta: transformMoveDelta,
    });
    connectorController = new ConnectorGestureController({
''',
    '''    const moveController = new MoveGestureController({
      screenToDocument,
      transformDelta: transformMoveDelta,
    });
    transformController = new TransformGestureController({
      screenToDocument,
      minimumSizeMm: 1,
      rotationSnapDeg: 15,
    });
    connectorController = new ConnectorGestureController({
''',
    "candidate transform controller",
)

candidate = replace_once(
    candidate,
    '''      resolveElementIds: (event) => {
        if (connectorTool !== null || endpointController?.isActive) {
          return null;
        }
        if (event.target?.closest?.(`[${CONNECTOR_ENDPOINT_HANDLE_ATTRIBUTE}]`)) {
          return null;
        }
        const target = event.target?.closest?.("[data-element-id]");
        if (!target || !svg.contains(target) || target.closest(`[${MOVE_OVERLAY_ATTRIBUTE}]`)) {
          clearSelection();
          return null;
        }
        const elementId = target.getAttribute("data-element-id");
        if (!elementId) {
          return null;
        }
        applySelection([elementId]);
        return [elementId];
      },
''',
    '''      resolveElementIds: (event) => {
        if (
          connectorTool !== null ||
          endpointController?.isActive ||
          transformController?.isActive
        ) {
          return null;
        }
        if (
          event.target?.closest?.(`[${CONNECTOR_ENDPOINT_HANDLE_ATTRIBUTE}]`) ||
          event.target?.closest?.(`[${TRANSFORM_HANDLE_ATTRIBUTE}]`)
        ) {
          return null;
        }
        const target = event.target?.closest?.("[data-element-id]");
        const hitElementId =
          target && svg.contains(target) && !target.closest(`[${MOVE_OVERLAY_ATTRIBUTE}]`)
            ? target.getAttribute("data-element-id")
            : null;
        const resolved = resolveMouseSelection({
          currentIds: selectedElementIds,
          hitElementId,
          shiftKey: event.shiftKey,
          ctrlKey: event.ctrlKey,
          metaKey: event.metaKey,
        });
        applySelection(resolved.selectionIds);
        return resolved.moveElementIds;
      },
''',
    "candidate mouse selection policy",
)

candidate = replace_once(
    candidate,
    '''    const disposeEndpoint = bindConnectorEndpointPointerSurface(svg, {
''',
    '''    const disposeTransform = bindTransformPointerSurface(svg, {
      controller: transformController,
      resolveHandle: (event) => {
        if (
          connectorTool !== null ||
          endpointController?.isActive ||
          transformCommitPending ||
          !transformSelection ||
          !presentationGeometry
        ) {
          return null;
        }
        const handle = event.target?.closest?.(`[${TRANSFORM_HANDLE_ATTRIBUTE}]`);
        if (!handle || !svg.contains(handle)) {
          return null;
        }
        if (
          handle.getAttribute(TRANSFORM_ELEMENT_ID_ATTRIBUTE) !== transformSelection.elementId
        ) {
          return null;
        }
        return {
          handle: handle.getAttribute(TRANSFORM_HANDLE_ATTRIBUTE),
          selection: {
            ...transformSelection,
            pageSize: {
              width: presentationGeometry.widthMm,
              height: presentationGeometry.heightMm,
            },
          },
        };
      },
      onOverlay: (preview) => {
        if (preview === null) {
          removeTransformPreview(svg);
          renderTransformEditor(
            svg,
            connectorTool === null ? transformSelection : null,
          );
          return;
        }
        removeTransformEditor(svg);
        renderTransformPreview(svg, preview);
      },
      onCommit: (commit) => {
        renderTransformPreview(svg, { ...commit, kind: "transform-preview" });
        transformCommitPending = true;
        Promise.resolve()
          .then(() => commitTransform(commit))
          .catch((error) => {
            removeTransformPreview(svg);
            renderTransformEditor(
              svg,
              connectorTool === null ? transformSelection : null,
            );
            onError(error);
          })
          .finally(() => {
            transformCommitPending = false;
          });
      },
      onError,
    });

    const disposeEndpoint = bindConnectorEndpointPointerSurface(svg, {
''',
    "candidate transform pointer binding",
)

candidate = replace_once(
    candidate,
    '''    disposePointerBinding = () => {
      disposeEndpoint();
      disposeConnector();
      disposeMove();
      endpointController = null;
      connectorController = null;
    };
''',
    '''    disposePointerBinding = () => {
      disposeEndpoint();
      disposeConnector();
      disposeTransform();
      disposeMove();
      endpointController = null;
      connectorController = null;
      transformController = null;
    };
''',
    "candidate transform disposal",
)

candidate = replace_once(
    candidate,
    '''      removeMoveOverlay(svg);
      removeSnapGuides(svg);
      removeConnectorPreview(svg);
''',
    '''      removeMoveOverlay(svg);
      removeSnapGuides(svg);
      removeTransformPreview(svg);
      removeTransformEditor(svg);
      removeConnectorPreview(svg);
''',
    "candidate presentation transform cleanup",
)

candidate = replace_once(
    candidate,
    '''      connectorController = null;
      endpointController = null;
      endpointSelection = null;
      clearSelection({ notify: false });
''',
    '''      connectorController = null;
      endpointController = null;
      transformController = null;
      endpointSelection = null;
      transformSelection = null;
      clearSelection({ notify: false });
''',
    "candidate presentation transform state reset",
)

candidate = replace_once(
    candidate,
    '''      renderConnectorToolPorts(
        svg,
        next !== null && interactionSettings.snappingEnabled ? presentationGeometry : null,
      );
      renderConnectorEndpointEditor(svg, next === null ? endpointSelection : null, presentationGeometry);
      return connectorTool;
    },

    setConnectorEndpointSelection(selection) {
''',
    '''      renderConnectorToolPorts(
        svg,
        next !== null && interactionSettings.snappingEnabled ? presentationGeometry : null,
      );
      renderTransformEditor(svg, next === null ? transformSelection : null);
      renderConnectorEndpointEditor(svg, next === null ? endpointSelection : null, presentationGeometry);
      return connectorTool;
    },

    setTransformSelection(selection) {
      cancelTransformGesture(svg, transformController);
      transformSelection = normalizeTransformSelection(selection);
      renderTransformEditor(
        svg,
        connectorTool === null ? transformSelection : null,
      );
      return transformSelection;
    },

    cancelTransformGesture() {
      const cancelled = cancelTransformGesture(svg, transformController);
      if (cancelled) {
        renderTransformEditor(
          svg,
          connectorTool === null ? transformSelection : null,
        );
      }
      return cancelled;
    },

    setConnectorEndpointSelection(selection) {
''',
    "candidate transform public API",
)

# clear() and dispose() contain the same cleanup sequence. Replace both remaining occurrences.
old_cleanup = '''      removeMoveOverlay(svg);
      removeSnapGuides(svg);
      removeConnectorPreview(svg);
      removeConnectorToolPorts(svg);
      removeConnectorEndpointPreview(svg);
      removeConnectorEndpointEditor(svg);
      connectorController = null;
      endpointController = null;
      endpointSelection = null;
'''
new_cleanup = '''      removeMoveOverlay(svg);
      removeSnapGuides(svg);
      removeTransformPreview(svg);
      removeTransformEditor(svg);
      removeConnectorPreview(svg);
      removeConnectorToolPorts(svg);
      removeConnectorEndpointPreview(svg);
      removeConnectorEndpointEditor(svg);
      connectorController = null;
      endpointController = null;
      transformController = null;
      endpointSelection = null;
      transformSelection = null;
'''
count = candidate.count(old_cleanup)
if count != 2:
    raise RuntimeError(f"candidate clear/dispose cleanup: expected 2 anchors, found {count}")
candidate = candidate.replace(old_cleanup, new_cleanup)

transform_helpers = r'''
function removeTransformEditor(svg) {
  if (!svg) {
    return;
  }
  for (const editor of svg.querySelectorAll(`[${TRANSFORM_EDITOR_ATTRIBUTE}]`)) {
    editor.remove();
  }
}

function removeTransformPreview(svg) {
  if (!svg) {
    return;
  }
  for (const preview of svg.querySelectorAll(`[${TRANSFORM_PREVIEW_ATTRIBUTE}]`)) {
    preview.remove();
  }
  for (const source of svg.querySelectorAll(`[${TRANSFORM_SOURCE_ATTRIBUTE}]`)) {
    source.removeAttribute(TRANSFORM_SOURCE_ATTRIBUTE);
  }
}

function renderTransformEditor(svg, selection) {
  removeTransformEditor(svg);
  if (!svg || !selection) {
    return;
  }
  const bounds = selection.boundsMm;
  const rotationDeg = selection.rotationDeg;
  const group = document.createElementNS(SVG_NS, "g");
  group.setAttribute(TRANSFORM_EDITOR_ATTRIBUTE, "true");
  group.setAttribute("aria-hidden", "true");

  const frame = document.createElementNS(SVG_NS, "rect");
  frame.setAttribute(TRANSFORM_FRAME_ATTRIBUTE, "true");
  frame.setAttribute("x", formatFinite(bounds.x));
  frame.setAttribute("y", formatFinite(bounds.y));
  frame.setAttribute("width", formatFinite(bounds.width));
  frame.setAttribute("height", formatFinite(bounds.height));
  frame.setAttribute("pointer-events", "none");
  applyElementRotation(frame, bounds, rotationDeg);
  group.append(frame);

  const mmPerPx = endpointMmPerPx(svg);
  const radius = Math.max(mmPerPx * 4, 0.45);
  const rotateOffset = Math.max(mmPerPx * 18, 4);
  const handleAxes = {
    nw: [-1, -1],
    n: [0, -1],
    ne: [1, -1],
    e: [1, 0],
    se: [1, 1],
    s: [0, 1],
    sw: [-1, 1],
    w: [-1, 0],
  };
  for (const [handleName, [axisX, axisY]] of Object.entries(handleAxes)) {
    const point = rotateLocalPoint(
      bounds,
      rotationDeg,
      (axisX * bounds.width) / 2,
      (axisY * bounds.height) / 2,
    );
    group.append(
      createTransformHandle(selection.elementId, handleName, point, radius),
    );
  }

  const topCenter = rotateLocalPoint(bounds, rotationDeg, 0, -bounds.height / 2);
  const rotatePoint = rotateLocalPoint(
    bounds,
    rotationDeg,
    0,
    -bounds.height / 2 - rotateOffset,
  );
  const guide = document.createElementNS(SVG_NS, "line");
  guide.setAttribute(TRANSFORM_ROTATE_GUIDE_ATTRIBUTE, "true");
  guide.setAttribute("x1", formatFinite(topCenter.x));
  guide.setAttribute("y1", formatFinite(topCenter.y));
  guide.setAttribute("x2", formatFinite(rotatePoint.x));
  guide.setAttribute("y2", formatFinite(rotatePoint.y));
  guide.setAttribute("pointer-events", "none");
  group.append(guide);
  group.append(
    createTransformHandle(selection.elementId, "rotate", rotatePoint, radius * 1.1),
  );
  svg.append(group);
}

function createTransformHandle(elementId, handleName, point, radius) {
  const handle = document.createElementNS(SVG_NS, "circle");
  handle.setAttribute(TRANSFORM_HANDLE_ATTRIBUTE, handleName);
  handle.setAttribute(TRANSFORM_ELEMENT_ID_ATTRIBUTE, elementId);
  handle.setAttribute("cx", formatFinite(point.x));
  handle.setAttribute("cy", formatFinite(point.y));
  handle.setAttribute("r", formatFinite(radius));
  handle.setAttribute("tabindex", "-1");
  return handle;
}

function renderTransformPreview(svg, preview) {
  removeTransformPreview(svg);
  if (!svg || preview?.kind !== "transform-preview") {
    return;
  }
  const source = findRenderableElement(svg, preview.elementId);
  source?.setAttribute(TRANSFORM_SOURCE_ATTRIBUTE, "true");
  const frame = document.createElementNS(SVG_NS, "rect");
  frame.setAttribute(TRANSFORM_PREVIEW_ATTRIBUTE, "true");
  frame.setAttribute("x", formatFinite(preview.boundsMm.x));
  frame.setAttribute("y", formatFinite(preview.boundsMm.y));
  frame.setAttribute("width", formatFinite(preview.boundsMm.width));
  frame.setAttribute("height", formatFinite(preview.boundsMm.height));
  frame.setAttribute("pointer-events", "none");
  frame.setAttribute("aria-hidden", "true");
  applyElementRotation(frame, preview.boundsMm, preview.rotationDeg);
  svg.append(frame);
}

function normalizeTransformSelection(selection) {
  if (selection == null || selection.geometryEditable === false) {
    return null;
  }
  const bounds = selection.boundsMm;
  if (
    typeof selection.elementId !== "string" ||
    selection.elementId.length === 0 ||
    !Number.isFinite(bounds?.x) ||
    !Number.isFinite(bounds?.y) ||
    !Number.isFinite(bounds?.width) ||
    !Number.isFinite(bounds?.height) ||
    bounds.width <= 0 ||
    bounds.height <= 0 ||
    !Number.isFinite(selection.rotationDeg)
  ) {
    throw new TypeError("transform selection must contain finite editable geometry");
  }
  return Object.freeze({
    elementId: selection.elementId,
    boundsMm: Object.freeze({ ...bounds }),
    rotationDeg: selection.rotationDeg,
  });
}

function rotateLocalPoint(bounds, rotationDeg, localX, localY) {
  const centerX = bounds.x + bounds.width / 2;
  const centerY = bounds.y + bounds.height / 2;
  const radians = (rotationDeg * Math.PI) / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  return {
    x: centerX + localX * cos - localY * sin,
    y: centerY + localX * sin + localY * cos,
  };
}

function applyElementRotation(element, bounds, rotationDeg) {
  if (rotationDeg === 0) {
    return;
  }
  element.setAttribute(
    "transform",
    `rotate(${formatFinite(rotationDeg)} ${formatFinite(bounds.x + bounds.width / 2)} ${formatFinite(bounds.y + bounds.height / 2)})`,
  );
}

function cancelTransformGesture(svg, controller) {
  if (!controller?.isActive) {
    removeTransformPreview(svg);
    return false;
  }
  const pointerId = controller.activePointerId;
  const cancelled = controller.cancel(pointerId);
  if (cancelled && pointerId !== null) {
    try {
      if (svg?.hasPointerCapture?.(pointerId) !== false) {
        svg?.releasePointerCapture?.(pointerId);
      }
    } catch {
      // Pointer capture can already be gone during teardown.
    }
  }
  removeTransformPreview(svg);
  return cancelled;
}

'''

candidate = replace_once(
    candidate,
    '''function removeConnectorPreview(svg) {
''',
    transform_helpers + '''function removeConnectorPreview(svg) {
''',
    "candidate transform helper insertion",
)

candidate_path.write_text(candidate, encoding="utf-8")

app_path = Path("apps/desktop/ui/app.js")
app = app_path.read_text(encoding="utf-8")

app = replace_once(
    app,
    '''const svgSurface = createSvgSurface(elements.canvasPage, {
  commitMove: commitSvgMove,
  commitConnector: commitSvgConnector,
''',
    '''const svgSurface = createSvgSurface(elements.canvasPage, {
  commitMove: commitSvgMove,
  commitTransform: commitSvgTransform,
  commitConnector: commitSvgConnector,
''',
    "app transform callback",
)

app = replace_once(
    app,
    '''  if (!primary) {
    svgSurface.setConnectorEndpointSelection(null);
    elements.selectionPropertiesForm.hidden = true;
    return;
  }

  svgSurface.setConnectorEndpointSelection(
    primary.connector
      ? { elementId: primary.elementId, ...primary.connector }
      : null,
  );
''',
    '''  if (!primary) {
    svgSurface.setTransformSelection(null);
    svgSurface.setConnectorEndpointSelection(null);
    elements.selectionPropertiesForm.hidden = true;
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
''',
    "app transform selection",
)

app = replace_once(
    app,
    '''async function commitSvgConnector(commit) {
''',
    '''async function commitSvgTransform(commit) {
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
''',
    "app transform commit",
)

app = replace_once(
    app,
    '''    if (svgSurface.cancelConnectorEndpointGesture()) {
      setStatus('Connector endpoint edit cancelled');
      event.preventDefault();
      event.stopPropagation();
      return;
    }
''',
    '''    if (svgSurface.cancelTransformGesture()) {
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
''',
    "app transform escape",
)

app_path.write_text(app, encoding="utf-8")

styles_path = Path("apps/desktop/ui/styles.css")
styles = styles_path.read_text(encoding="utf-8")
transform_styles = r'''
.candidate-svg-document [data-ddn-transform-editor] {
  pointer-events: none;
}

.candidate-svg-document [data-ddn-transform-frame],
.candidate-svg-document [data-ddn-transform-preview] {
  fill: none;
  stroke: var(--selection);
  stroke-width: 1.5px;
  stroke-dasharray: 4 2;
  vector-effect: non-scaling-stroke;
}

.candidate-svg-document [data-ddn-transform-preview] {
  stroke-width: 2px;
  opacity: 0.95;
}

.candidate-svg-document [data-ddn-transform-source="true"] {
  opacity: 0.28;
}

.candidate-svg-document [data-ddn-transform-rotate-guide] {
  stroke: var(--selection);
  stroke-width: 1.2px;
  vector-effect: non-scaling-stroke;
}

.candidate-svg-document [data-ddn-transform-handle] {
  fill: var(--surface);
  stroke: var(--selection);
  stroke-width: 1.8px;
  vector-effect: non-scaling-stroke;
  pointer-events: all;
}

.candidate-svg-document [data-ddn-transform-handle]:hover {
  fill: var(--selection-soft);
  stroke-width: 2.4px;
}

.candidate-svg-document [data-ddn-transform-handle="nw"],
.candidate-svg-document [data-ddn-transform-handle="se"] {
  cursor: nwse-resize;
}

.candidate-svg-document [data-ddn-transform-handle="ne"],
.candidate-svg-document [data-ddn-transform-handle="sw"] {
  cursor: nesw-resize;
}

.candidate-svg-document [data-ddn-transform-handle="n"],
.candidate-svg-document [data-ddn-transform-handle="s"] {
  cursor: ns-resize;
}

.candidate-svg-document [data-ddn-transform-handle="e"],
.candidate-svg-document [data-ddn-transform-handle="w"] {
  cursor: ew-resize;
}

.candidate-svg-document [data-ddn-transform-handle="rotate"] {
  cursor: grab;
}

'''
styles = replace_once(
    styles,
    '''.candidate-svg-document [data-ddn-connector-endpoint-editor] {
''',
    transform_styles + '''.candidate-svg-document [data-ddn-connector-endpoint-editor] {
''',
    "transform styles",
)
styles_path.write_text(styles, encoding="utf-8")
