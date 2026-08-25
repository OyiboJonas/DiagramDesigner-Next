from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"missing patch anchor: {label}")
    return text.replace(old, new, 1)


# SVG candidate interaction surface.
path = Path("apps/desktop/ui/candidate-svg-surface.mjs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''import {\n  MoveGestureController,\n  bindMovePointerSurface,\n} from "./editor-interaction/move-gesture.mjs";\n''',
    '''import {\n  ConnectorEndpointGestureController,\n  bindConnectorEndpointPointerSurface,\n  nearestPortTarget,\n} from "./editor-interaction/connector-endpoint-gesture.mjs";\nimport {\n  MoveGestureController,\n  bindMovePointerSurface,\n} from "./editor-interaction/move-gesture.mjs";\n''',
    "endpoint gesture imports",
)
text = replace_once(
    text,
    '''const CONNECTOR_PREVIEW_ATTRIBUTE = "data-ddn-connector-preview";\n''',
    '''const CONNECTOR_PREVIEW_ATTRIBUTE = "data-ddn-connector-preview";\nconst CONNECTOR_ENDPOINT_EDITOR_ATTRIBUTE = "data-ddn-connector-endpoint-editor";\nconst CONNECTOR_ENDPOINT_HANDLE_ATTRIBUTE = "data-ddn-connector-endpoint-handle";\nconst CONNECTOR_ENDPOINT_PREVIEW_ATTRIBUTE = "data-ddn-connector-endpoint-preview";\nconst CONNECTOR_PORT_HANDLE_ATTRIBUTE = "data-ddn-connector-port-handle";\nconst CONNECTOR_PORT_SNAPPED_ATTRIBUTE = "data-ddn-connector-port-snapped";\nconst CONNECTOR_ELEMENT_ID_ATTRIBUTE = "data-ddn-connector-element-id";\nconst CONNECTOR_PORT_ELEMENT_ID_ATTRIBUTE = "data-ddn-port-element-id";\nconst CONNECTOR_PORT_ID_ATTRIBUTE = "data-ddn-port-id";\n''',
    "endpoint overlay constants",
)
text = replace_once(
    text,
    '''    commitMove,\n    commitConnector = () => {},\n    onError = (error) => {\n''',
    '''    commitMove,\n    commitConnector = () => {},\n    commitConnectorEndpoint = () => {},\n    onError = (error) => {\n''',
    "endpoint commit option",
)
text = replace_once(
    text,
    '''  if (typeof commitConnector !== "function") {\n    throw new TypeError("commitConnector must be a function");\n  }\n''',
    '''  if (typeof commitConnector !== "function") {\n    throw new TypeError("commitConnector must be a function");\n  }\n  if (typeof commitConnectorEndpoint !== "function") {\n    throw new TypeError("commitConnectorEndpoint must be a function");\n  }\n''',
    "endpoint commit validation",
)
text = replace_once(
    text,
    '''  let connectorController = null;\n  let connectorTool = null;\n  let connectorCommitPending = false;\n  let selectedElementIds = [];\n''',
    '''  let connectorController = null;\n  let connectorTool = null;\n  let connectorCommitPending = false;\n  let endpointController = null;\n  let endpointSelection = null;\n  let endpointCommitPending = false;\n  let selectedElementIds = [];\n''',
    "endpoint interaction state",
)
text = replace_once(
    text,
    '''    const changed = !sameIds(selectedElementIds, applied);\n    selectedElementIds = applied;\n    if (changed && notify) {\n''',
    '''    const changed = !sameIds(selectedElementIds, applied);\n    selectedElementIds = applied;\n    if (\n      endpointSelection &&\n      (applied.length !== 1 || applied[0] !== endpointSelection.elementId)\n    ) {\n      endpointSelection = null;\n      removeConnectorEndpointEditor(svg);\n    }\n    if (changed && notify) {\n''',
    "selection clears stale endpoint editor",
)
text = replace_once(
    text,
    '''    connectorController = new ConnectorGestureController({\n      screenToDocument,\n      minimumLengthMm: 0.5,\n    });\n\n    const disposeMove = bindMovePointerSurface(svg, {\n''',
    '''    connectorController = new ConnectorGestureController({\n      screenToDocument,\n      minimumLengthMm: 0.5,\n    });\n    endpointController = new ConnectorEndpointGestureController({\n      screenToDocument,\n      resolvePortTarget: ({ pointMm, elementId }) => {\n        if (!presentationGeometry || !interactionSettings.snappingEnabled) {\n          return null;\n        }\n        const thresholdMm = endpointSnapThresholdMm(svg, interactionSettings.snapThresholdPx);\n        return nearestPortTarget(\n          pointMm,\n          presentationGeometry.portTargets,\n          thresholdMm,\n          { excludeElementId: elementId },\n        );\n      },\n    });\n\n    const disposeMove = bindMovePointerSurface(svg, {\n''',
    "endpoint controller binding",
)
text = replace_once(
    text,
    '''        if (connectorTool !== null) {\n          return null;\n        }\n        const target = event.target?.closest?.("[data-element-id]");\n''',
    '''        if (connectorTool !== null || endpointController?.isActive) {\n          return null;\n        }\n        if (event.target?.closest?.(`[${CONNECTOR_ENDPOINT_HANDLE_ATTRIBUTE}]`)) {\n          return null;\n        }\n        const target = event.target?.closest?.("[data-element-id]");\n''',
    "move ignores endpoint handles",
)
text = replace_once(
    text,
    '''    const disposeConnector = bindConnectorPointerSurface(svg, {\n''',
    '''    const disposeEndpoint = bindConnectorEndpointPointerSurface(svg, {\n      controller: endpointController,\n      resolveHandle: (event) => {\n        if (connectorTool !== null || endpointCommitPending || !endpointSelection) {\n          return null;\n        }\n        const handle = event.target?.closest?.(`[${CONNECTOR_ENDPOINT_HANDLE_ATTRIBUTE}]`);\n        if (!handle || !svg.contains(handle)) {\n          return null;\n        }\n        const elementId = handle.getAttribute(CONNECTOR_ELEMENT_ID_ATTRIBUTE);\n        const side = handle.getAttribute(CONNECTOR_ENDPOINT_HANDLE_ATTRIBUTE);\n        if (elementId !== endpointSelection.elementId || (side !== "start" && side !== "end")) {\n          return null;\n        }\n        return {\n          elementId,\n          side,\n          connectorKind: endpointSelection.kind,\n          startEndpoint: endpointSelection.start,\n          endEndpoint: endpointSelection.end,\n        };\n      },\n      onOverlay: (preview) => renderConnectorEndpointPreview(svg, preview),\n      onCommit: (commit) => {\n        const preview = endpointCommitPreview(endpointSelection, commit);\n        renderConnectorEndpointPreview(svg, preview);\n        endpointCommitPending = true;\n        Promise.resolve()\n          .then(() => commitConnectorEndpoint(commit))\n          .catch((error) => {\n            removeConnectorEndpointPreview(svg);\n            renderConnectorEndpointEditor(svg, endpointSelection, presentationGeometry);\n            onError(error);\n          })\n          .finally(() => {\n            endpointCommitPending = false;\n          });\n      },\n      onError,\n    });\n\n    const disposeConnector = bindConnectorPointerSurface(svg, {\n''',
    "endpoint pointer binding",
)
text = replace_once(
    text,
    '''    disposePointerBinding = () => {\n      disposeConnector();\n      disposeMove();\n      connectorController = null;\n    };\n''',
    '''    disposePointerBinding = () => {\n      disposeEndpoint();\n      disposeConnector();\n      disposeMove();\n      endpointController = null;\n      connectorController = null;\n    };\n''',
    "endpoint binding disposal",
)
text = replace_once(
    text,
    '''      removeSnapGuides(svg);\n      removeConnectorPreview(svg);\n      connectorController = null;\n      clearSelection({ notify: false });\n''',
    '''      removeSnapGuides(svg);\n      removeConnectorPreview(svg);\n      removeConnectorEndpointPreview(svg);\n      removeConnectorEndpointEditor(svg);\n      connectorController = null;\n      endpointController = null;\n      endpointSelection = null;\n      clearSelection({ notify: false });\n''',
    "presentation endpoint cleanup",
)
text = replace_once(
    text,
    '''        snapElements: Object.freeze(\n          normalizeSnapElements(presentation.snapElements ?? []).filter((element) =>\n            Boolean(findRenderableElement(svg, element.elementId)),\n          ),\n        ),\n      });\n''',
    '''        snapElements: Object.freeze(\n          normalizeSnapElements(presentation.snapElements ?? []).filter((element) =>\n            Boolean(findRenderableElement(svg, element.elementId)),\n          ),\n        ),\n        portTargets: Object.freeze(normalizePortTargets(presentation.portTargets ?? [])),\n      });\n''',
    "presentation port target geometry",
)
text = replace_once(
    text,
    '''      if (connectorController?.isActive) {\n        connectorController.cancel();\n      }\n      removeConnectorPreview(svg);\n      connectorTool = next;\n''',
    '''      if (connectorController?.isActive) {\n        connectorController.cancel();\n      }\n      cancelEndpointGesture(svg, endpointController);\n      removeConnectorPreview(svg);\n      connectorTool = next;\n''',
    "connector tool cancels endpoint edit",
)
text = replace_once(
    text,
    '''      } else {\n        host.setAttribute("data-connector-tool", next);\n      }\n      return connectorTool;\n    },\n\n    setInteractionSettings(settings) {\n''',
    '''      } else {\n        host.setAttribute("data-connector-tool", next);\n      }\n      renderConnectorEndpointEditor(svg, next === null ? endpointSelection : null, presentationGeometry);\n      return connectorTool;\n    },\n\n    setConnectorEndpointSelection(selection) {\n      cancelEndpointGesture(svg, endpointController);\n      endpointSelection = normalizeConnectorEndpointSelection(selection);\n      renderConnectorEndpointEditor(\n        svg,\n        connectorTool === null ? endpointSelection : null,\n        presentationGeometry,\n      );\n      return endpointSelection;\n    },\n\n    cancelConnectorEndpointGesture() {\n      const cancelled = cancelEndpointGesture(svg, endpointController);\n      if (cancelled) {\n        renderConnectorEndpointEditor(\n          svg,\n          connectorTool === null ? endpointSelection : null,\n          presentationGeometry,\n        );\n      }\n      return cancelled;\n    },\n\n    setInteractionSettings(settings) {\n''',
    "endpoint selection surface API",
)
# clear and dispose each contain similar cleanup. Replace remaining two occurrences after first presentation replacement.
for label in ["clear endpoint cleanup", "dispose endpoint cleanup"]:
    old = '''      removeSnapGuides(svg);\n      removeConnectorPreview(svg);\n      connectorController = null;\n      clearSelection();\n'''
    new = '''      removeSnapGuides(svg);\n      removeConnectorPreview(svg);\n      removeConnectorEndpointPreview(svg);\n      removeConnectorEndpointEditor(svg);\n      connectorController = null;\n      endpointController = null;\n      endpointSelection = null;\n      clearSelection();\n'''
    text = replace_once(text, old, new, label)
text = replace_once(
    text,
    '''function removeSnapGuides(svg) {\n''',
    r'''function removeConnectorEndpointEditor(svg) {
  if (!svg) {
    return;
  }
  for (const editor of svg.querySelectorAll(`[${CONNECTOR_ENDPOINT_EDITOR_ATTRIBUTE}]`)) {
    editor.remove();
  }
}

function removeConnectorEndpointPreview(svg) {
  if (!svg) {
    return;
  }
  for (const preview of svg.querySelectorAll(`[${CONNECTOR_ENDPOINT_PREVIEW_ATTRIBUTE}]`)) {
    preview.remove();
  }
  for (const port of svg.querySelectorAll(`[${CONNECTOR_PORT_SNAPPED_ATTRIBUTE}]`)) {
    port.removeAttribute(CONNECTOR_PORT_SNAPPED_ATTRIBUTE);
  }
}

function renderConnectorEndpointEditor(svg, selection, geometry) {
  removeConnectorEndpointEditor(svg);
  if (!svg || !selection || !geometry) {
    return;
  }
  const mmPerPx = endpointMmPerPx(svg);
  const group = document.createElementNS(SVG_NS, "g");
  group.setAttribute(CONNECTOR_ENDPOINT_EDITOR_ATTRIBUTE, "true");
  group.setAttribute("aria-hidden", "true");

  for (const port of geometry.portTargets) {
    if (port.elementId === selection.elementId) {
      continue;
    }
    const handle = document.createElementNS(SVG_NS, "circle");
    handle.setAttribute(CONNECTOR_PORT_HANDLE_ATTRIBUTE, "true");
    handle.setAttribute(CONNECTOR_PORT_ELEMENT_ID_ATTRIBUTE, port.elementId);
    handle.setAttribute(CONNECTOR_PORT_ID_ATTRIBUTE, port.portId);
    handle.setAttribute("cx", formatFinite(port.positionMm.x));
    handle.setAttribute("cy", formatFinite(port.positionMm.y));
    handle.setAttribute("r", formatFinite(Math.max(mmPerPx * 3.25, 0.35)));
    handle.setAttribute("pointer-events", "none");
    group.append(handle);
  }

  for (const side of ["start", "end"]) {
    const endpoint = selection[side];
    const handle = document.createElementNS(SVG_NS, "circle");
    handle.setAttribute(CONNECTOR_ENDPOINT_HANDLE_ATTRIBUTE, side);
    handle.setAttribute(CONNECTOR_ELEMENT_ID_ATTRIBUTE, selection.elementId);
    handle.setAttribute("cx", formatFinite(endpoint.positionMm.x));
    handle.setAttribute("cy", formatFinite(endpoint.positionMm.y));
    handle.setAttribute("r", formatFinite(Math.max(mmPerPx * 5, 0.55)));
    handle.setAttribute("tabindex", "-1");
    group.append(handle);
  }
  svg.append(group);
}

function renderConnectorEndpointPreview(svg, preview) {
  removeConnectorEndpointPreview(svg);
  if (!svg || preview?.kind !== "connector-endpoint-preview") {
    return;
  }
  const element =
    preview.connectorKind === "orthogonal"
      ? document.createElementNS(SVG_NS, "polyline")
      : document.createElementNS(SVG_NS, "line");
  element.setAttribute(CONNECTOR_ENDPOINT_PREVIEW_ATTRIBUTE, preview.connectorKind);
  element.setAttribute("pointer-events", "none");
  element.setAttribute("aria-hidden", "true");
  if (preview.connectorKind === "orthogonal") {
    const points = buildOrthogonalPreviewPoints(preview.startMm, preview.endMm);
    element.setAttribute(
      "points",
      points.map((point) => `${formatFinite(point.x)},${formatFinite(point.y)}`).join(" "),
    );
  } else {
    element.setAttribute("x1", formatFinite(preview.startMm.x));
    element.setAttribute("y1", formatFinite(preview.startMm.y));
    element.setAttribute("x2", formatFinite(preview.endMm.x));
    element.setAttribute("y2", formatFinite(preview.endMm.y));
  }
  svg.append(element);
  if (preview.connection) {
    markSnappedPort(svg, preview.connection);
  }
}

function markSnappedPort(svg, connection) {
  for (const port of svg.querySelectorAll(`[${CONNECTOR_PORT_HANDLE_ATTRIBUTE}]`)) {
    if (
      port.getAttribute(CONNECTOR_PORT_ELEMENT_ID_ATTRIBUTE) === connection.elementId &&
      port.getAttribute(CONNECTOR_PORT_ID_ATTRIBUTE) === connection.portId
    ) {
      port.setAttribute(CONNECTOR_PORT_SNAPPED_ATTRIBUTE, "true");
      break;
    }
  }
}

function endpointCommitPreview(selection, commit) {
  if (!selection || !commit) {
    return null;
  }
  const startMm = commit.side === "start" ? commit.positionMm : selection.start.positionMm;
  const endMm = commit.side === "end" ? commit.positionMm : selection.end.positionMm;
  return {
    kind: "connector-endpoint-preview",
    connectorKind: selection.kind,
    startMm,
    endMm,
    positionMm: commit.positionMm,
    connection: commit.connection,
  };
}

function cancelEndpointGesture(svg, controller) {
  if (!controller?.isActive) {
    removeConnectorEndpointPreview(svg);
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
  removeConnectorEndpointPreview(svg);
  return cancelled;
}

function endpointSnapThresholdMm(svg, thresholdPx) {
  return endpointMmPerPx(svg) * thresholdPx;
}

function endpointMmPerPx(svg) {
  if (!svg) {
    return 1;
  }
  const rect = svg.getBoundingClientRect();
  const viewBox = svg.viewBox?.baseVal;
  if (!viewBox || rect.width <= 0 || rect.height <= 0) {
    return 1;
  }
  return Math.max(viewBox.width / rect.width, viewBox.height / rect.height);
}

function removeSnapGuides(svg) {
''',
    "endpoint overlay helpers",
)
text = replace_once(
    text,
    '''function normalizeInteractionSettings(current, patch) {\n''',
    r'''function normalizePortTargets(ports) {
  if (!Array.isArray(ports)) {
    throw new TypeError("candidate portTargets must be an array");
  }
  const normalized = [];
  const seen = new Set();
  for (const port of ports) {
    const elementId = port?.elementId;
    const portId = port?.portId;
    const positionMm = port?.positionMm;
    if (typeof elementId !== "string" || elementId.length === 0) {
      throw new TypeError("candidate port target element IDs must be non-empty strings");
    }
    if (typeof portId !== "string" || portId.length === 0) {
      throw new TypeError("candidate port target port IDs must be non-empty strings");
    }
    if (!Number.isFinite(positionMm?.x) || !Number.isFinite(positionMm?.y)) {
      throw new TypeError("candidate port target positions must be finite");
    }
    const key = `${elementId}:${portId}`;
    if (seen.has(key)) {
      throw new TypeError(`duplicate candidate port target: ${key}`);
    }
    seen.add(key);
    normalized.push(
      Object.freeze({
        elementId,
        portId,
        positionMm: Object.freeze({ x: positionMm.x, y: positionMm.y }),
      }),
    );
  }
  return normalized;
}

function normalizeConnectorEndpointSelection(selection) {
  if (selection == null) {
    return null;
  }
  if (typeof selection.elementId !== "string" || selection.elementId.length === 0) {
    throw new TypeError("connector endpoint selection requires an elementId");
  }
  if (selection.kind !== "straight" && selection.kind !== "orthogonal") {
    throw new TypeError("connector endpoint selection kind must be straight or orthogonal");
  }
  return Object.freeze({
    elementId: selection.elementId,
    kind: selection.kind,
    start: normalizeConnectorEndpoint(selection.start),
    end: normalizeConnectorEndpoint(selection.end),
  });
}

function normalizeConnectorEndpoint(endpoint) {
  if (!Number.isFinite(endpoint?.positionMm?.x) || !Number.isFinite(endpoint?.positionMm?.y)) {
    throw new TypeError("connector endpoint position must be finite");
  }
  const connection = endpoint.connection == null
    ? null
    : Object.freeze({
        elementId: endpoint.connection.elementId,
        portId: endpoint.connection.portId,
      });
  if (
    connection &&
    (typeof connection.elementId !== "string" ||
      connection.elementId.length === 0 ||
      typeof connection.portId !== "string" ||
      connection.portId.length === 0)
  ) {
    throw new TypeError("connector endpoint connection IDs must be non-empty strings");
  }
  return Object.freeze({
    positionMm: Object.freeze({ x: endpoint.positionMm.x, y: endpoint.positionMm.y }),
    connection,
  });
}

function normalizeInteractionSettings(current, patch) {
''',
    "endpoint geometry normalization",
)
path.write_text(text, encoding="utf-8")


# app.js: bridge selection properties and endpoint commits.
path = Path("apps/desktop/ui/app.js")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''  commitMove: commitSvgMove,\n  commitConnector: commitSvgConnector,\n  onSelectionChange: (elementIds) => {\n''',
    '''  commitMove: commitSvgMove,\n  commitConnector: commitSvgConnector,\n  commitConnectorEndpoint: commitSvgConnectorEndpoint,\n  onSelectionChange: (elementIds) => {\n''',
    "surface endpoint commit callback",
)
text = replace_once(
    text,
    '''  const primary = details?.primary ?? null;\n  elements.applyProperties.disabled =\n    !primary || (primary.geometryEditable === false && primary.textEditable !== true);\n  if (!primary) {\n    elements.selectionPropertiesForm.hidden = true;\n    return;\n  }\n\n  elements.selectionPropertiesForm.hidden = false;\n''',
    '''  const primary = details?.primary ?? null;\n  elements.applyProperties.disabled =\n    !primary || (primary.geometryEditable === false && primary.textEditable !== true);\n  if (!primary) {\n    svgSurface.setConnectorEndpointSelection(null);\n    elements.selectionPropertiesForm.hidden = true;\n    return;\n  }\n\n  svgSurface.setConnectorEndpointSelection(\n    primary.connector\n      ? { elementId: primary.elementId, ...primary.connector }\n      : null,\n  );\n  elements.selectionPropertiesForm.hidden = false;\n''',
    "selection endpoint handles",
)
text = replace_once(
    text,
    '''async function syncRecovery() {\n''',
    r'''async function commitSvgConnectorEndpoint(commit) {
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
''',
    "endpoint IPC commit",
)
text = replace_once(
    text,
    '''  (event) => {\n    if (event.key === 'Escape' && connectorTool !== null && !elements.recoveryDialog.open) {\n      setConnectorTool(null);\n      event.preventDefault();\n      event.stopPropagation();\n    }\n  },\n''',
    '''  (event) => {\n    if (event.key !== 'Escape' || elements.recoveryDialog.open) {\n      return;\n    }\n    if (svgSurface.cancelConnectorEndpointGesture()) {\n      setStatus('Connector endpoint edit cancelled');\n      event.preventDefault();\n      event.stopPropagation();\n      return;\n    }\n    if (connectorTool !== null) {\n      setConnectorTool(null);\n      event.preventDefault();\n      event.stopPropagation();\n    }\n  },\n''',
    "Escape endpoint cancellation",
)
path.write_text(text, encoding="utf-8")


# Styles: endpoint/port editing overlays use existing theme tokens.
path = Path("apps/desktop/ui/styles.css")
text = path.read_text(encoding="utf-8")
anchor = '''.candidate-svg-document [data-ddn-snap-source="object"] {\n  opacity: 0.95;\n}\n\n'''
addition = r'''.candidate-svg-document [data-ddn-connector-endpoint-editor] {
  pointer-events: none;
}

.candidate-svg-document [data-ddn-connector-endpoint-handle] {
  fill: var(--surface);
  stroke: var(--selection);
  stroke-width: 2px;
  vector-effect: non-scaling-stroke;
  pointer-events: all;
  cursor: crosshair;
}

.candidate-svg-document [data-ddn-connector-endpoint-handle]:hover {
  fill: var(--selection-soft);
  stroke-width: 2.5px;
}

.candidate-svg-document [data-ddn-connector-port-handle] {
  fill: var(--surface);
  stroke: var(--selection);
  stroke-width: 1.4px;
  opacity: 0.72;
  vector-effect: non-scaling-stroke;
}

.candidate-svg-document [data-ddn-connector-port-snapped="true"] {
  fill: var(--selection);
  opacity: 1;
  stroke-width: 2.2px;
}

.candidate-svg-document [data-ddn-connector-endpoint-preview] {
  fill: none;
  stroke: var(--selection);
  stroke-width: 2px;
  stroke-dasharray: 5 3;
  opacity: 0.92;
  vector-effect: non-scaling-stroke;
}

'''
text = replace_once(text, anchor, anchor + addition, "endpoint overlay styles")
path.write_text(text, encoding="utf-8")
