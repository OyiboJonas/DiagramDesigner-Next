from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def replace_count(text: str, old: str, new: str, expected: int, label: str) -> str:
    count = text.count(old)
    if count != expected:
        raise RuntimeError(f"{label}: expected {expected} anchors, found {count}")
    return text.replace(old, new)


def patch_connector_gesture(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    text = replace_once(
        text,
        "  #screenToDocument;\n  #minimumLengthMm;\n  #active = null;\n",
        "  #screenToDocument;\n  #minimumLengthMm;\n  #resolvePortTarget;\n  #active = null;\n",
        f"{path}: fields",
    )
    text = replace_once(
        text,
        """  constructor({ screenToDocument, minimumLengthMm = 0.5 } = {}) {
    if (typeof screenToDocument !== 'function') {
      throw new ConnectorGestureError('screenToDocument must be a function');
    }
    if (!Number.isFinite(minimumLengthMm) || minimumLengthMm < 0) {
      throw new ConnectorGestureError('minimumLengthMm must be finite and non-negative');
    }
    this.#screenToDocument = screenToDocument;
    this.#minimumLengthMm = minimumLengthMm;
  }
""",
        """  constructor({ screenToDocument, minimumLengthMm = 0.5, resolvePortTarget = () => null } = {}) {
    if (typeof screenToDocument !== 'function') {
      throw new ConnectorGestureError('screenToDocument must be a function');
    }
    if (!Number.isFinite(minimumLengthMm) || minimumLengthMm < 0) {
      throw new ConnectorGestureError('minimumLengthMm must be finite and non-negative');
    }
    if (typeof resolvePortTarget !== 'function') {
      throw new ConnectorGestureError('resolvePortTarget must be a function');
    }
    this.#screenToDocument = screenToDocument;
    this.#minimumLengthMm = minimumLengthMm;
    this.#resolvePortTarget = resolvePortTarget;
  }
""",
        f"{path}: constructor",
    )
    text = replace_once(
        text,
        """    const kind = normalizeConnectorKind(connectorKind);
    const startMm = validateDocumentPoint(this.#screenToDocument(validateScreenPoint(screenPoint)));
    this.#active = {
      pointerId,
      connectorKind: kind,
      startMm,
      endMm: startMm,
    };
""",
        """    const kind = normalizeConnectorKind(connectorKind);
    const rawStartMm = validateDocumentPoint(
      this.#screenToDocument(validateScreenPoint(screenPoint)),
    );
    const startEndpoint = this.#resolveEndpoint(rawStartMm, 'start');
    this.#active = {
      pointerId,
      connectorKind: kind,
      startMm: startEndpoint.positionMm,
      endMm: startEndpoint.positionMm,
      startConnection: startEndpoint.connection,
      endConnection: startEndpoint.connection,
    };
""",
        f"{path}: begin",
    )
    text = replace_once(
        text,
        """    this.#active.endMm = validateDocumentPoint(
      this.#screenToDocument(validateScreenPoint(screenPoint)),
    );
    return this.#preview();
""",
        """    const rawEndMm = validateDocumentPoint(
      this.#screenToDocument(validateScreenPoint(screenPoint)),
    );
    const endEndpoint = this.#resolveEndpoint(rawEndMm, 'end');
    this.#active.endMm = endEndpoint.positionMm;
    this.#active.endConnection = endEndpoint.connection;
    return this.#preview();
""",
        f"{path}: update",
    )
    text = replace_once(
        text,
        """      connectorKind: finished.connectorKind,
      startMm: Object.freeze({ ...finished.startMm }),
      endMm: Object.freeze({ ...finished.endMm }),
    });
""",
        """      connectorKind: finished.connectorKind,
      startMm: Object.freeze({ ...finished.startMm }),
      endMm: Object.freeze({ ...finished.endMm }),
      startConnection: freezeConnection(finished.startConnection),
      endConnection: freezeConnection(finished.endConnection),
    });
""",
        f"{path}: finish",
    )
    text = replace_once(
        text,
        """  #preview() {
""",
        """  #resolveEndpoint(pointMm, side) {
    const target = normalizePortTarget(
      this.#resolvePortTarget(
        Object.freeze({
          pointMm: Object.freeze({ ...pointMm }),
          side,
        }),
      ),
    );
    if (target === null) {
      return {
        positionMm: Object.freeze({ ...pointMm }),
        connection: null,
      };
    }
    return {
      positionMm: target.positionMm,
      connection: Object.freeze({ elementId: target.elementId, portId: target.portId }),
    };
  }

  #preview() {
""",
        f"{path}: resolver method",
    )
    text = replace_once(
        text,
        """      connectorKind: this.#active.connectorKind,
      startMm: Object.freeze({ ...this.#active.startMm }),
      endMm: Object.freeze({ ...this.#active.endMm }),
    });
""",
        """      connectorKind: this.#active.connectorKind,
      startMm: Object.freeze({ ...this.#active.startMm }),
      endMm: Object.freeze({ ...this.#active.endMm }),
      startConnection: freezeConnection(this.#active.startConnection),
      endConnection: freezeConnection(this.#active.endConnection),
    });
""",
        f"{path}: preview",
    )
    text = replace_once(
        text,
        """function distance(left, right) {
""",
        """function normalizePortTarget(target) {
  if (target == null) {
    return null;
  }
  const elementId = target.elementId;
  const portId = target.portId;
  if (typeof elementId !== 'string' || elementId.length === 0) {
    throw new ConnectorGestureError('resolved port target requires a non-empty elementId');
  }
  if (typeof portId !== 'string' || portId.length === 0) {
    throw new ConnectorGestureError('resolved port target requires a non-empty portId');
  }
  return Object.freeze({
    elementId,
    portId,
    positionMm: Object.freeze(validateDocumentPoint(target.positionMm)),
  });
}

function freezeConnection(connection) {
  return connection == null
    ? null
    : Object.freeze({ elementId: connection.elementId, portId: connection.portId });
}

function distance(left, right) {
""",
        f"{path}: port normalizer",
    )
    path.write_text(text, encoding="utf-8")


for module_path in [
    Path("apps/desktop/ui/editor-interaction/connector-gesture.mjs"),
    Path("web/editor-interaction/connector-gesture.mjs"),
]:
    patch_connector_gesture(module_path)

# Keep the production and independently tested interaction modules byte-identical.
desktop_module = Path("apps/desktop/ui/editor-interaction/connector-gesture.mjs").read_text(encoding="utf-8")
web_module = Path("web/editor-interaction/connector-gesture.mjs").read_text(encoding="utf-8")
if desktop_module != web_module:
    raise RuntimeError("connector gesture production/test copies diverged")

# Extend deterministic gesture expectations and cover connected creation.
test_path = Path("web/editor-interaction/connector-gesture.test.mjs")
tests = test_path.read_text(encoding="utf-8")
tests = replace_once(
    tests,
    """    connectorKind: 'straight',
    startMm: { x: 10, y: 10 },
    endMm: { x: 10, y: 10 },
  });
""",
    """    connectorKind: 'straight',
    startMm: { x: 10, y: 10 },
    endMm: { x: 10, y: 10 },
    startConnection: null,
    endConnection: null,
  });
""",
    "connector test begin expectation",
)
tests = replace_once(
    tests,
    """    connectorKind: 'straight',
    startMm: { x: 10, y: 10 },
    endMm: { x: 30, y: 25 },
  });
""",
    """    connectorKind: 'straight',
    startMm: { x: 10, y: 10 },
    endMm: { x: 30, y: 25 },
    startConnection: null,
    endConnection: null,
  });
""",
    "connector test commit expectation",
)
tests += """

test('connector gesture can resolve both creation endpoints to canonical ports in one intent', () => {
  const controller = new ConnectorGestureController({
    screenToDocument: ({ xPx, yPx }) => ({ x: xPx, y: yPx }),
    resolvePortTarget: ({ side }) =>
      side === 'start'
        ? { elementId: 'shape-a', portId: 'port-right', positionMm: { x: 20, y: 30 } }
        : { elementId: 'shape-b', portId: 'port-left', positionMm: { x: 80, y: 40 } },
  });

  const begin = controller.begin({
    pointerId: 9,
    screenPoint: { xPx: 18, yPx: 31 },
    connectorKind: 'straight',
  });
  assert.deepEqual(begin.startMm, { x: 20, y: 30 });
  assert.deepEqual(begin.startConnection, { elementId: 'shape-a', portId: 'port-right' });

  const commit = controller.finish({
    pointerId: 9,
    screenPoint: { xPx: 82, yPx: 39 },
  });
  assert.deepEqual(commit, {
    kind: 'create-connector',
    connectorKind: 'straight',
    startMm: { x: 20, y: 30 },
    endMm: { x: 80, y: 40 },
    startConnection: { elementId: 'shape-a', portId: 'port-right' },
    endConnection: { elementId: 'shape-b', portId: 'port-left' },
  });
});

test('connector gesture keeps an endpoint free when its port resolver has no hit', () => {
  const controller = new ConnectorGestureController({
    screenToDocument: ({ xPx, yPx }) => ({ x: xPx, y: yPx }),
    resolvePortTarget: ({ side }) =>
      side === 'start'
        ? { elementId: 'shape-a', portId: 'port-right', positionMm: { x: 20, y: 30 } }
        : null,
  });
  controller.begin({
    pointerId: 10,
    screenPoint: { xPx: 20, yPx: 30 },
    connectorKind: 'orthogonal',
  });
  const commit = controller.finish({
    pointerId: 10,
    screenPoint: { xPx: 70, yPx: 75 },
  });
  assert.deepEqual(commit.startConnection, { elementId: 'shape-a', portId: 'port-right' });
  assert.equal(commit.endConnection, null);
  assert.deepEqual(commit.endMm, { x: 70, y: 75 });
});
"""
test_path.write_text(tests, encoding="utf-8")

# Tauri: accept optional connections and canonicalize them against the active-layer port query.
rust_path = Path("apps/desktop/src-tauri/src/lib.rs")
rust = rust_path.read_text(encoding="utf-8")
rust = replace_once(
    rust,
    """struct CreateConnectorRequest {
    kind: ConnectorKind,
    start_mm: Point,
    end_mm: Point,
}
""",
    """struct CreateConnectorRequest {
    kind: ConnectorKind,
    start_mm: Point,
    end_mm: Point,
    start_connection: Option<Connection>,
    end_connection: Option<Connection>,
}
""",
    "create connector DTO",
)
rust = replace_once(
    rust,
    """    let start_mm = clamp_connector_point(request.start_mm, page_size)?;
    let end_mm = clamp_connector_point(request.end_mm, page_size)?;
    let distance_mm = (end_mm.x - start_mm.x).hypot(end_mm.y - start_mm.y);
    if distance_mm < 0.5 {
        return Err(CommandError::new(
            "connector_too_short",
            "Drag at least 0.5 mm to create a connector.",
        ));
    }

    let element_id = ElementId::new();
    let connector = Connector {
        start: Endpoint {
            position_mm: start_mm,
            connection: None,
        },
        end: Endpoint {
            position_mm: end_mm,
            connection: None,
        },
        start_marker: MarkerStyle::None,
        end_marker: MarkerStyle::None,
        line_style: LineStyle::Solid,
        secondary_color: None,
    };
""",
    """    let ports = document
        .session
        .active_page_layer_ports()
        .map_err(|error| CommandError::new("connector_ports_failed", error.to_string()))?;
    let start = connector_creation_endpoint(
        request.start_mm,
        request.start_connection,
        page_size,
        &ports,
    )?;
    let end = connector_creation_endpoint(
        request.end_mm,
        request.end_connection,
        page_size,
        &ports,
    )?;
    let start_mm = start.position_mm;
    let end_mm = end.position_mm;
    let distance_mm = (end_mm.x - start_mm.x).hypot(end_mm.y - start_mm.y);
    if distance_mm < 0.5 {
        return Err(CommandError::new(
            "connector_too_short",
            "Drag at least 0.5 mm to create a connector.",
        ));
    }

    let element_id = ElementId::new();
    let connector = Connector {
        start,
        end,
        start_marker: MarkerStyle::None,
        end_marker: MarkerStyle::None,
        line_style: LineStyle::Solid,
        secondary_color: None,
    };
""",
    "create connector endpoint construction",
)
rust = replace_once(
    rust,
    """fn connector_bounds(start_mm: Point, end_mm: Point) -> Rect {
""",
    """fn connector_creation_endpoint(
    position_mm: Point,
    connection: Option<Connection>,
    page_size: Size,
    ports: &[app_core::ConnectorPortPosition],
) -> Result<Endpoint, CommandError> {
    let Some(connection) = connection else {
        return Ok(Endpoint {
            position_mm: clamp_connector_point(position_mm, page_size)?,
            connection: None,
        });
    };
    let port = ports
        .iter()
        .find(|port| port.element_id == connection.element_id && port.port_id == connection.port_id)
        .ok_or_else(|| {
            CommandError::new(
                "connector_port_missing",
                "The requested connector port is no longer available on the active editable layer.",
            )
        })?;
    Ok(Endpoint {
        position_mm: port.position_mm,
        connection: Some(connection),
    })
}

fn connector_bounds(start_mm: Point, end_mm: Point) -> Rect {
""",
    "connector creation endpoint helper",
)
rust_path.write_text(rust, encoding="utf-8")

# Desktop command bridge forwards the optional semantic connections in the same create request.
app_path = Path("apps/desktop/ui/app.js")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    """        kind: commit.connectorKind,
        startMm: { ...commit.startMm },
        endMm: { ...commit.endMm },
      },
""",
    """        kind: commit.connectorKind,
        startMm: { ...commit.startMm },
        endMm: { ...commit.endMm },
        startConnection: commit.startConnection
          ? { elementId: commit.startConnection.elementId, portId: commit.startConnection.portId }
          : null,
        endConnection: commit.endConnection
          ? { elementId: commit.endConnection.elementId, portId: commit.endConnection.portId }
          : null,
      },
""",
    "desktop connector create bridge",
)
app_path.write_text(app, encoding="utf-8")

# SVG surface: expose eligible ports while drawing and use the same screen-space hit threshold.
surface_path = Path("apps/desktop/ui/candidate-svg-surface.mjs")
surface = surface_path.read_text(encoding="utf-8")
surface = replace_once(
    surface,
    """const CONNECTOR_PREVIEW_ATTRIBUTE = "data-ddn-connector-preview";
const CONNECTOR_ENDPOINT_EDITOR_ATTRIBUTE = "data-ddn-connector-endpoint-editor";
""",
    """const CONNECTOR_PREVIEW_ATTRIBUTE = "data-ddn-connector-preview";
const CONNECTOR_TOOL_PORTS_ATTRIBUTE = "data-ddn-connector-tool-ports";
const CONNECTOR_ENDPOINT_EDITOR_ATTRIBUTE = "data-ddn-connector-endpoint-editor";
""",
    "surface tool port constant",
)
surface = replace_once(
    surface,
    """    connectorController = new ConnectorGestureController({
      screenToDocument,
      minimumLengthMm: 0.5,
    });
""",
    """    connectorController = new ConnectorGestureController({
      screenToDocument,
      minimumLengthMm: 0.5,
      resolvePortTarget: ({ pointMm }) => {
        if (!presentationGeometry || !interactionSettings.snappingEnabled) {
          return null;
        }
        return nearestPortTarget(
          pointMm,
          presentationGeometry.portTargets,
          endpointSnapThresholdMm(svg, interactionSettings.snapThresholdPx),
        );
      },
    });
""",
    "surface connector resolver",
)
surface = replace_count(
    surface,
    """      removeConnectorPreview(svg);
      removeConnectorEndpointPreview(svg);
      removeConnectorEndpointEditor(svg);
""",
    """      removeConnectorPreview(svg);
      removeConnectorToolPorts(svg);
      removeConnectorEndpointPreview(svg);
      removeConnectorEndpointEditor(svg);
""",
    3,
    "surface teardown tool ports",
)
surface = replace_once(
    surface,
    """      bindPointerInteraction();
      applySelection(previousSelection, { notify: false });
    },
""",
    """      bindPointerInteraction();
      applySelection(previousSelection, { notify: false });
      renderConnectorToolPorts(
        svg,
        connectorTool !== null && interactionSettings.snappingEnabled ? presentationGeometry : null,
      );
    },
""",
    "surface presentation tool ports",
)
surface = replace_once(
    surface,
    """      if (next === null) {
        host.removeAttribute("data-connector-tool");
      } else {
        host.setAttribute("data-connector-tool", next);
      }
      renderConnectorEndpointEditor(svg, next === null ? endpointSelection : null, presentationGeometry);
""",
    """      if (next === null) {
        host.removeAttribute("data-connector-tool");
      } else {
        host.setAttribute("data-connector-tool", next);
      }
      renderConnectorToolPorts(
        svg,
        next !== null && interactionSettings.snappingEnabled ? presentationGeometry : null,
      );
      renderConnectorEndpointEditor(svg, next === null ? endpointSelection : null, presentationGeometry);
""",
    "surface tool switch ports",
)
surface = replace_once(
    surface,
    """      interactionSettings = normalizeInteractionSettings(interactionSettings, settings);
      applyGridStyle(host, presentationGeometry, interactionSettings);
      removeSnapGuides(svg);
      return Object.freeze({ ...interactionSettings });
""",
    """      interactionSettings = normalizeInteractionSettings(interactionSettings, settings);
      applyGridStyle(host, presentationGeometry, interactionSettings);
      removeSnapGuides(svg);
      renderConnectorToolPorts(
        svg,
        connectorTool !== null && interactionSettings.snappingEnabled ? presentationGeometry : null,
      );
      return Object.freeze({ ...interactionSettings });
""",
    "surface snapping toggle ports",
)
surface = replace_once(
    surface,
    """  for (const preview of svg.querySelectorAll(`[${CONNECTOR_PREVIEW_ATTRIBUTE}]`)) {
    preview.remove();
  }
}

function renderConnectorPreview(svg, preview) {
""",
    """  for (const preview of svg.querySelectorAll(`[${CONNECTOR_PREVIEW_ATTRIBUTE}]`)) {
    preview.remove();
  }
  for (const port of svg.querySelectorAll(`[${CONNECTOR_PORT_SNAPPED_ATTRIBUTE}]`)) {
    port.removeAttribute(CONNECTOR_PORT_SNAPPED_ATTRIBUTE);
  }
}

function renderConnectorPreview(svg, preview) {
""",
    "surface preview snapped cleanup",
)
surface = replace_once(
    surface,
    """  }
  svg.append(element);
}

function removeConnectorEndpointEditor(svg) {
""",
    """  }
  svg.append(element);
  if (preview.startConnection) {
    markSnappedPort(svg, preview.startConnection);
  }
  if (preview.endConnection) {
    markSnappedPort(svg, preview.endConnection);
  }
}

function removeConnectorToolPorts(svg) {
  if (!svg) {
    return;
  }
  for (const ports of svg.querySelectorAll(`[${CONNECTOR_TOOL_PORTS_ATTRIBUTE}]`)) {
    ports.remove();
  }
}

function renderConnectorToolPorts(svg, geometry) {
  removeConnectorToolPorts(svg);
  if (!svg || !geometry) {
    return;
  }
  const group = document.createElementNS(SVG_NS, "g");
  group.setAttribute(CONNECTOR_TOOL_PORTS_ATTRIBUTE, "true");
  group.setAttribute("pointer-events", "none");
  group.setAttribute("aria-hidden", "true");
  const radius = Math.max(endpointMmPerPx(svg) * 3.25, 0.35);
  for (const port of geometry.portTargets) {
    const handle = document.createElementNS(SVG_NS, "circle");
    handle.setAttribute(CONNECTOR_PORT_HANDLE_ATTRIBUTE, "true");
    handle.setAttribute(CONNECTOR_PORT_ELEMENT_ID_ATTRIBUTE, port.elementId);
    handle.setAttribute(CONNECTOR_PORT_ID_ATTRIBUTE, port.portId);
    handle.setAttribute("cx", formatFinite(port.positionMm.x));
    handle.setAttribute("cy", formatFinite(port.positionMm.y));
    handle.setAttribute("r", formatFinite(radius));
    handle.setAttribute("pointer-events", "none");
    group.append(handle);
  }
  svg.append(group);
}

function removeConnectorEndpointEditor(svg) {
""",
    "surface tool port renderer",
)
surface_path.write_text(surface, encoding="utf-8")

print("Prepared connector creation port attachment product changes.")
