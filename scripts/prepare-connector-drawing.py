from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    if text.count(old) != 1:
        raise SystemExit(f"Expected one anchor in {path}, found {text.count(old)}: {old[:120]!r}")
    write(path, text.replace(old, new, 1))


def replace_section(path: str, start: str, end: str, replacement: str) -> None:
    text = read(path)
    start_index = text.find(start)
    if start_index < 0:
        raise SystemExit(f"Start anchor not found in {path}: {start!r}")
    end_index = text.find(end, start_index)
    if end_index < 0:
        raise SystemExit(f"End anchor not found in {path}: {end!r}")
    if text.find(start, start_index + 1) >= 0:
        raise SystemExit(f"Start anchor is not unique in {path}: {start!r}")
    write(path, text[:start_index] + replacement + text[end_index:])


def append_once(path: str, marker: str, extra: str) -> None:
    text = read(path)
    if marker in text:
        return
    write(path, text.rstrip() + "\n\n" + extra.strip() + "\n")


# --- Desktop Rust semantic connector creation boundary ---
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """use next_domain::{\n    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,\n    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Point, Rect, RichTextDocument,\n    RichTextToken, Scene, Size, TextBlock, TextHorizontalAlignment, TextLayout, TextStyle,\n    TextVerticalAlignment,\n};\n""",
    """use next_domain::{\n    AnchorSet, Connector, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,\n    ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle, NextArtifact, Page,\n    PageId, Point, Rect, RichTextDocument, RichTextToken, Scene, Size, TextBlock,\n    TextHorizontalAlignment, TextLayout, TextStyle, TextVerticalAlignment,\n};\n""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """struct CreateBasicElementRequest {\n    kind: BasicElementKind,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct UpdateElementPropertiesRequest {\n""",
    """struct CreateBasicElementRequest {\n    kind: BasicElementKind,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\nenum ConnectorKind {\n    Straight,\n    Orthogonal,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct CreateConnectorRequest {\n    kind: ConnectorKind,\n    start_mm: Point,\n    end_mm: Point,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct UpdateElementPropertiesRequest {\n""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """    text: Option<String>,\n    text_editable: bool,\n}\n""",
    """    text: Option<String>,\n    text_editable: bool,\n    geometry_editable: bool,\n}\n""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn delete_selection(state: State<'_, DesktopState>) -> Result<ElementEditResultDto, CommandError> {\n""",
    """    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn create_connector(\n    request: CreateConnectorRequest,\n    state: State<'_, DesktopState>,\n) -> Result<ElementEditResultDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let (target, page_size) = {\n        let session = document.session.session();\n        let page_id = session.active_page_id().ok_or_else(|| {\n            CommandError::new(\"no_active_page\", \"The current document has no active page.\")\n        })?;\n        let layer_id = document.session.active_page_layer_id().ok_or_else(|| {\n            CommandError::new(\n                \"no_active_page_layer\",\n                \"Choose a page-local layer before drawing a connector.\",\n            )\n        })?;\n        let page = session\n            .document()\n            .pages\n            .iter()\n            .find(|page| page.id == page_id)\n            .ok_or_else(|| CommandError::new(\"page_missing\", \"The active page no longer exists.\"))?;\n        let layer = page\n            .layers\n            .iter()\n            .find(|layer| layer.id == layer_id)\n            .ok_or_else(|| CommandError::new(\"layer_missing\", \"The active layer no longer exists.\"))?;\n        if !layer.visible {\n            return Err(CommandError::new(\n                \"connector_layer_hidden\",\n                \"Connectors can be drawn only on a visible layer.\",\n            ));\n        }\n        if layer.locked {\n            return Err(CommandError::new(\n                \"connector_layer_locked\",\n                \"Unlock the active layer before drawing a connector.\",\n            ));\n        }\n        let target = session.active_layer().ok_or_else(|| {\n            CommandError::new(\"no_active_layer\", \"The current document has no active layer.\")\n        })?;\n        (target, page.size_mm)\n    };\n\n    let start_mm = clamp_connector_point(request.start_mm, page_size)?;\n    let end_mm = clamp_connector_point(request.end_mm, page_size)?;\n    let distance_mm = (end_mm.x - start_mm.x).hypot(end_mm.y - start_mm.y);\n    if distance_mm < 0.5 {\n        return Err(CommandError::new(\n            \"connector_too_short\",\n            \"Drag at least 0.5 mm to create a connector.\",\n        ));\n    }\n\n    let element_id = ElementId::new();\n    let connector = Connector {\n        start: Endpoint {\n            position_mm: start_mm,\n            connection: None,\n        },\n        end: Endpoint {\n            position_mm: end_mm,\n            connection: None,\n        },\n        start_marker: MarkerStyle::None,\n        end_marker: MarkerStyle::None,\n        line_style: LineStyle::Solid,\n        secondary_color: None,\n    };\n    let (name, kind) = match request.kind {\n        ConnectorKind::Straight => (\n            \"Connector\".to_owned(),\n            ElementKind::StraightConnector { connector },\n        ),\n        ConnectorKind::Orthogonal => (\n            \"Orthogonal connector\".to_owned(),\n            ElementKind::OrthogonalConnector {\n                connector,\n                corner_radius_mm: 0.0,\n            },\n        ),\n    };\n    let element = Element {\n        id: element_id,\n        name,\n        bounds_mm: connector_bounds(start_mm, end_mm),\n        rotation_deg: 0.0,\n        anchors: AnchorSet::default(),\n        ports: Vec::new(),\n        style_id: None,\n        text: None,\n        kind,\n        import: None,\n    };\n\n    document\n        .session\n        .create_element(target, element, None)\n        .map_err(|error| CommandError::new(\"connector_create_failed\", error.to_string()))?;\n    document\n        .session\n        .set_selection([element_id])\n        .map_err(|error| CommandError::new(\"selection_failed\", error.to_string()))?;\n    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn delete_selection(state: State<'_, DesktopState>) -> Result<ElementEditResultDto, CommandError> {\n""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """fn update_element_properties(\n    request: UpdateElementPropertiesRequest,\n    state: State<'_, DesktopState>,\n) -> Result<ElementEditResultDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let text_update = if let Some(text) = request.text {\n""",
    """fn update_element_properties(\n    request: UpdateElementPropertiesRequest,\n    state: State<'_, DesktopState>,\n) -> Result<ElementEditResultDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let existing = find_element(document.session.session().document(), request.element_id)\n        .ok_or_else(|| {\n            CommandError::new(\n                \"element_properties_missing\",\n                \"The selected element no longer exists in the current document.\",\n            )\n        })?;\n    if !element_geometry_editable(&existing.kind) {\n        return Err(CommandError::new(\n            \"element_geometry_requires_dedicated_tool\",\n            \"This element uses a dedicated geometry tool and cannot be resized in the basic inspector.\",\n        ));\n    }\n    let text_update = if let Some(text) = request.text {\n""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """        text,\n        text_editable,\n    }\n}\n\nfn element_type_name(kind: &ElementKind) -> &'static str {\n""",
    """        text,\n        text_editable,\n        geometry_editable: element_geometry_editable(&element.kind),\n    }\n}\n\nfn element_geometry_editable(kind: &ElementKind) -> bool {\n    !matches!(\n        kind,\n        ElementKind::StraightConnector { .. }\n            | ElementKind::OrthogonalConnector { .. }\n            | ElementKind::Group { .. }\n    )\n}\n\nfn element_type_name(kind: &ElementKind) -> &'static str {\n""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """fn simple_text_block(text: &str, style: TextStyle, layout: Option<TextLayout>) -> TextBlock {\n""",
    """fn clamp_connector_point(point: Point, page_size: Size) -> Result<Point, CommandError> {\n    if !point.x.is_finite() || !point.y.is_finite() {\n        return Err(CommandError::new(\n            \"invalid_connector_geometry\",\n            \"Connector endpoints must contain finite coordinates.\",\n        ));\n    }\n    Ok(Point {\n        x: point.x.clamp(0.0, page_size.width),\n        y: point.y.clamp(0.0, page_size.height),\n    })\n}\n\nfn connector_bounds(start_mm: Point, end_mm: Point) -> Rect {\n    Rect {\n        x: start_mm.x.min(end_mm.x),\n        y: start_mm.y.min(end_mm.y),\n        width: (start_mm.x - end_mm.x).abs().max(0.1),\n        height: (start_mm.y - end_mm.y).abs().max(0.1),\n    }\n}\n\nfn simple_text_block(text: &str, style: TextStyle, layout: Option<TextLayout>) -> TextBlock {\n""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """            selection_properties,\n            create_basic_element,\n            delete_selection,\n""",
    """            selection_properties,\n            create_basic_element,\n            create_connector,\n            delete_selection,\n""",
)

replace_once(
    "apps/desktop/src-tauri/build.rs",
    """            \"selection_properties\",\n            \"create_basic_element\",\n            \"delete_selection\",\n""",
    """            \"selection_properties\",\n            \"create_basic_element\",\n            \"create_connector\",\n            \"delete_selection\",\n""",
)

# --- Toolbar and inspector affordances ---
replace_once(
    "apps/desktop/ui/index.html",
    """          <button id=\"add-rectangle\" type=\"button\" title=\"Create a rectangle on the active layer\">Rectangle</button>\n          <button id=\"add-text\" type=\"button\" title=\"Create a text box on the active layer\">Text</button>\n          <button id=\"delete-selection\" type=\"button\" title=\"Delete the current selection\">Delete</button>\n""",
    """          <button id=\"add-rectangle\" type=\"button\" title=\"Create a rectangle on the active layer\">Rectangle</button>\n          <button id=\"add-text\" type=\"button\" title=\"Create a text box on the active layer\">Text</button>\n          <button id=\"draw-straight-connector\" class=\"toggle-button\" type=\"button\" aria-pressed=\"false\" title=\"Draw straight connectors\">Line</button>\n          <button id=\"draw-orthogonal-connector\" class=\"toggle-button\" type=\"button\" aria-pressed=\"false\" title=\"Draw orthogonal connectors\">Orthogonal</button>\n          <button id=\"delete-selection\" type=\"button\" title=\"Delete the current selection\">Delete</button>\n""",
)

replace_once(
    "apps/desktop/ui/index.html",
    """              <div class=\"property-grid\">\n                <label>X <input id=\"property-x\" type=\"number\" step=\"0.1\" /></label>\n                <label>Y <input id=\"property-y\" type=\"number\" step=\"0.1\" /></label>\n                <label>Width <input id=\"property-width\" type=\"number\" min=\"0.1\" step=\"0.1\" /></label>\n                <label>Height <input id=\"property-height\" type=\"number\" min=\"0.1\" step=\"0.1\" /></label>\n              </div>\n              <label class=\"property-field\">Rotation° <input id=\"property-rotation\" type=\"number\" step=\"0.1\" /></label>\n""",
    """              <div class=\"property-grid\">\n                <label>X <input id=\"property-x\" type=\"number\" step=\"0.1\" /></label>\n                <label>Y <input id=\"property-y\" type=\"number\" step=\"0.1\" /></label>\n                <label>Width <input id=\"property-width\" type=\"number\" min=\"0.1\" step=\"0.1\" /></label>\n                <label>Height <input id=\"property-height\" type=\"number\" min=\"0.1\" step=\"0.1\" /></label>\n              </div>\n              <label class=\"property-field\">Rotation° <input id=\"property-rotation\" type=\"number\" step=\"0.1\" /></label>\n              <p id=\"property-geometry-note\" class=\"property-note\" hidden>\n                This element uses a dedicated geometry tool. It can still be selected, moved and deleted.\n              </p>\n""",
)

replace_once(
    "apps/desktop/ui/index.html",
    """            Pointer movement, snapping and keyboard focus remain local to the presentation surface; only semantic selection and completed moves cross IPC.\n            Tab enters or leaves the canvas; arrow keys, Home and End navigate elements; Space or Enter selects; Escape clears selection.\n""",
    """            Pointer movement, connector previews, snapping and keyboard focus remain local to the presentation surface; only semantic selection, completed moves and completed connector creates cross IPC.\n            Tab enters or leaves the canvas; arrow keys, Home and End navigate elements; Space or Enter selects. Escape cancels an active connector tool or clears selection.\n""",
)

# --- App controller integration ---
replace_once(
    "apps/desktop/ui/app.js",
    """  addRectangle: document.querySelector('#add-rectangle'),\n  addText: document.querySelector('#add-text'),\n  deleteSelection: document.querySelector('#delete-selection'),\n""",
    """  addRectangle: document.querySelector('#add-rectangle'),\n  addText: document.querySelector('#add-text'),\n  drawStraightConnector: document.querySelector('#draw-straight-connector'),\n  drawOrthogonalConnector: document.querySelector('#draw-orthogonal-connector'),\n  deleteSelection: document.querySelector('#delete-selection'),\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """  propertyRotation: document.querySelector('#property-rotation'),\n  propertyTextField: document.querySelector('#property-text-field'),\n""",
    """  propertyRotation: document.querySelector('#property-rotation'),\n  propertyGeometryNote: document.querySelector('#property-geometry-note'),\n  propertyTextField: document.querySelector('#property-text-field'),\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """  elements.addRectangle,\n  elements.addText,\n  elements.deleteSelection,\n""",
    """  elements.addRectangle,\n  elements.addText,\n  elements.drawStraightConnector,\n  elements.drawOrthogonalConnector,\n  elements.deleteSelection,\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """let currentNavigation = null;\nlet isBusy = false;\nlet keyboardSurface = null;\n""",
    """let currentNavigation = null;\nlet connectorTool = null;\nlet isBusy = false;\nlet keyboardSurface = null;\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """const svgSurface = createSvgSurface(elements.canvasPage, {\n  commitMove: commitSvgMove,\n  onSelectionChange: (elementIds) => {\n""",
    """const svgSurface = createSvgSurface(elements.canvasPage, {\n  commitMove: commitSvgMove,\n  commitConnector: commitSvgConnector,\n  onSelectionChange: (elementIds) => {\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """  const layerEditable = Boolean(activeLayer?.visible && !activeLayer?.locked);\n  elements.addRectangle.disabled = isBusy || !layerEditable;\n  elements.addText.disabled = isBusy || !layerEditable;\n  elements.addRectangle.title = layerEditable\n    ? 'Create a rectangle on the active layer'\n    : 'Choose a visible, unlocked layer to create elements';\n  elements.addText.title = layerEditable\n    ? 'Create a text box on the active layer'\n    : 'Choose a visible, unlocked layer to create elements';\n""",
    """  const layerEditable = Boolean(activeLayer?.visible && !activeLayer?.locked);\n  elements.addRectangle.disabled = isBusy || !layerEditable;\n  elements.addText.disabled = isBusy || !layerEditable;\n  elements.drawStraightConnector.disabled = isBusy || !layerEditable;\n  elements.drawOrthogonalConnector.disabled = isBusy || !layerEditable;\n  if (!layerEditable && connectorTool !== null) {\n    setConnectorTool(null, { announce: false, clearSelection: false });\n  }\n  elements.addRectangle.title = layerEditable\n    ? 'Create a rectangle on the active layer'\n    : 'Choose a visible, unlocked layer to create elements';\n  elements.addText.title = layerEditable\n    ? 'Create a text box on the active layer'\n    : 'Choose a visible, unlocked layer to create elements';\n  elements.drawStraightConnector.title = layerEditable\n    ? 'Draw straight connectors'\n    : 'Choose a visible, unlocked layer to draw connectors';\n  elements.drawOrthogonalConnector.title = layerEditable\n    ? 'Draw orthogonal connectors'\n    : 'Choose a visible, unlocked layer to draw connectors';\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """function renderInteractionButtons() {\n  elements.toggleGrid.setAttribute('aria-pressed', String(interactionSettings.gridVisible));\n  elements.toggleSnap.setAttribute('aria-pressed', String(interactionSettings.snappingEnabled));\n}\n\nfunction applyInteractionSettings(message) {\n""",
    """function renderInteractionButtons() {\n  elements.toggleGrid.setAttribute('aria-pressed', String(interactionSettings.gridVisible));\n  elements.toggleSnap.setAttribute('aria-pressed', String(interactionSettings.snappingEnabled));\n  elements.drawStraightConnector.setAttribute('aria-pressed', String(connectorTool === 'straight'));\n  elements.drawOrthogonalConnector.setAttribute('aria-pressed', String(connectorTool === 'orthogonal'));\n}\n\nfunction setConnectorTool(kind, { announce = true, clearSelection = true } = {}) {\n  if (kind !== null && kind !== 'straight' && kind !== 'orthogonal') {\n    throw new TypeError(`Unsupported connector tool: ${String(kind)}`);\n  }\n  connectorTool = kind;\n  svgSurface.setConnectorTool(kind);\n  if (kind !== null && clearSelection) {\n    clearLocalSelection();\n  }\n  renderInteractionButtons();\n  if (announce) {\n    setStatus(\n      kind === null\n        ? 'Selection tool active'\n        : kind === 'straight'\n          ? 'Straight connector tool — drag on the page; Escape exits'\n          : 'Orthogonal connector tool — drag on the page; Escape exits',\n    );\n  }\n}\n\nfunction applyInteractionSettings(message) {\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """  elements.propertyRotation.value = String(primary.rotationDeg);\n\n  const hasText = primary.text !== null && primary.text !== undefined;\n""",
    """  elements.propertyRotation.value = String(primary.rotationDeg);\n  const geometryEditable = primary.geometryEditable !== false;\n  for (const input of [\n    elements.propertyX,\n    elements.propertyY,\n    elements.propertyWidth,\n    elements.propertyHeight,\n    elements.propertyRotation,\n  ]) {\n    input.disabled = !geometryEditable;\n  }\n  elements.propertyGeometryNote.hidden = geometryEditable;\n\n  const hasText = primary.text !== null && primary.text !== undefined;\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """  elements.applyProperties.disabled = !primary;\n  if (!primary) {\n""",
    """  elements.applyProperties.disabled =\n    !primary || (primary.geometryEditable === false && primary.textEditable !== true);\n  if (!primary) {\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """  const primary = currentSelectionProperties?.primary;\n  if (!invoke || !primary) {\n    return;\n  }\n  const numbers = {\n""",
    """  const primary = currentSelectionProperties?.primary;\n  if (!invoke || !primary) {\n    return;\n  }\n  if (primary.geometryEditable === false) {\n    setStatus('This element uses a dedicated geometry tool and cannot be resized in the basic inspector');\n    return;\n  }\n  const numbers = {\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """async function syncRecovery() {\n""",
    """async function commitSvgConnector(commit) {\n  if (!invoke) {\n    throw new Error('Tauri runtime not detected');\n  }\n  if (commit?.kind !== 'create-connector') {\n    throw new TypeError('SVG surface emitted an unsupported connector command');\n  }\n  const activeTool = connectorTool;\n  svgSurface.setConnectorTool(null);\n  setBusy(true);\n  try {\n    const result = await invoke('create_connector', {\n      request: {\n        kind: commit.connectorKind,\n        startMm: { ...commit.startMm },\n        endMm: { ...commit.endMm },\n      },\n    });\n    renderState(result.state);\n    await refreshPresentation({ preserveSelection: false });\n    svgSurface.setSelection(result.selectedElementIds ?? []);\n    keyboardSurface?.syncSelectionState(result.selectedElementIds ?? []);\n    await refreshSelectionProperties();\n    scheduleRecoverySync(250);\n    setStatus(commit.connectorKind === 'straight' ? 'Straight connector created' : 'Orthogonal connector created');\n    return result.state;\n  } finally {\n    setBusy(false);\n    if (connectorTool === activeTool && activeTool !== null) {\n      svgSurface.setConnectorTool(activeTool);\n    }\n  }\n}\n\nasync function syncRecovery() {\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """elements.newDocument.addEventListener('click', () => {\n  void runAction('new_document', undefined, () => 'New document created', {\n""",
    """elements.newDocument.addEventListener('click', () => {\n  setConnectorTool(null, { announce: false });\n  void runAction('new_document', undefined, () => 'New document created', {\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """elements.openDocument.addEventListener('click', () => {\n  void runAction(\n""",
    """elements.openDocument.addEventListener('click', () => {\n  setConnectorTool(null, { announce: false });\n  void runAction(\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """elements.addText.addEventListener('click', () => {\n  void createBasicElement('text');\n});\n\nelements.deleteSelection.addEventListener('click', () => {\n""",
    """elements.addText.addEventListener('click', () => {\n  void createBasicElement('text');\n});\n\nelements.drawStraightConnector.addEventListener('click', () => {\n  setConnectorTool(connectorTool === 'straight' ? null : 'straight');\n});\n\nelements.drawOrthogonalConnector.addEventListener('click', () => {\n  setConnectorTool(connectorTool === 'orthogonal' ? null : 'orthogonal');\n});\n\nelements.deleteSelection.addEventListener('click', () => {\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """elements.recoveryDialog.addEventListener('cancel', (event) => {\n  // Startup recovery must be an explicit Restore/Discard decision. Escape must not\n  // silently discard the only recovery snapshot.\n  event.preventDefault();\n});\n\nwindow.diagramDesignerNext = Object.freeze({\n""",
    """elements.recoveryDialog.addEventListener('cancel', (event) => {\n  // Startup recovery must be an explicit Restore/Discard decision. Escape must not\n  // silently discard the only recovery snapshot.\n  event.preventDefault();\n});\n\nwindow.addEventListener(\n  'keydown',\n  (event) => {\n    if (event.key === 'Escape' && connectorTool !== null && !elements.recoveryDialog.open) {\n      setConnectorTool(null);\n      event.preventDefault();\n      event.stopPropagation();\n    }\n  },\n  true,\n);\n\nwindow.diagramDesignerNext = Object.freeze({\n""",
)

# --- SVG surface owns transient connector gesture and preview ---
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """import {\n  MoveGestureController,\n  bindMovePointerSurface,\n} from \"./editor-interaction/move-gesture.mjs\";\nimport { snapMoveDelta } from \"./editor-interaction/snapping.mjs\";\n""",
    """import {\n  ConnectorGestureController,\n  bindConnectorPointerSurface,\n  buildOrthogonalPreviewPoints,\n  normalizeConnectorKind,\n} from \"./editor-interaction/connector-gesture.mjs\";\nimport {\n  MoveGestureController,\n  bindMovePointerSurface,\n} from \"./editor-interaction/move-gesture.mjs\";\nimport { snapMoveDelta } from \"./editor-interaction/snapping.mjs\";\n""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """const SNAP_GUIDES_ATTRIBUTE = \"data-ddn-snap-guides\";\nconst SNAP_GUIDE_ATTRIBUTE = \"data-ddn-snap-guide\";\n""",
    """const SNAP_GUIDES_ATTRIBUTE = \"data-ddn-snap-guides\";\nconst SNAP_GUIDE_ATTRIBUTE = \"data-ddn-snap-guide\";\nconst CONNECTOR_PREVIEW_ATTRIBUTE = \"data-ddn-connector-preview\";\n""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """  {\n    commitMove,\n    onError = (error) => {\n""",
    """  {\n    commitMove,\n    commitConnector = () => {},\n    onError = (error) => {\n""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """  if (typeof commitMove !== \"function\") {\n    throw new TypeError(\"commitMove must be a function\");\n  }\n  if (typeof onError !== \"function\" || typeof onSelectionChange !== \"function\") {\n""",
    """  if (typeof commitMove !== \"function\") {\n    throw new TypeError(\"commitMove must be a function\");\n  }\n  if (typeof commitConnector !== \"function\") {\n    throw new TypeError(\"commitConnector must be a function\");\n  }\n  if (typeof onError !== \"function\" || typeof onSelectionChange !== \"function\") {\n""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """  let svg = null;\n  let disposePointerBinding = null;\n  let selectedElementIds = [];\n""",
    """  let svg = null;\n  let disposePointerBinding = null;\n  let connectorController = null;\n  let connectorTool = null;\n  let selectedElementIds = [];\n""",
)

replace_section(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    "  const bindPointerInteraction = () => {",
    "\n\n  return Object.freeze({",
    """  const bindPointerInteraction = () => {\n    if (!svg) {\n      return;\n    }\n\n    const screenToDocument = ({ xPx, yPx }) => {\n      const rect = svg.getBoundingClientRect();\n      const viewBox = svg.viewBox?.baseVal;\n      if (!viewBox) {\n        throw new Error(\"candidate SVG does not expose a viewBox\");\n      }\n      return mapClientPointToViewBox(\n        rect,\n        { x: viewBox.x, y: viewBox.y, width: viewBox.width, height: viewBox.height },\n        { xPx, yPx },\n      );\n    };\n\n    const moveController = new MoveGestureController({\n      screenToDocument,\n      transformDelta: transformMoveDelta,\n    });\n    connectorController = new ConnectorGestureController({\n      screenToDocument,\n      minimumLengthMm: 0.5,\n    });\n\n    const disposeMove = bindMovePointerSurface(svg, {\n      controller: moveController,\n      resolveElementIds: (event) => {\n        if (connectorTool !== null) {\n          return null;\n        }\n        const target = event.target?.closest?.(\"[data-element-id]\");\n        if (!target || !svg.contains(target) || target.closest(`[${MOVE_OVERLAY_ATTRIBUTE}]`)) {\n          clearSelection();\n          return null;\n        }\n        const elementId = target.getAttribute(\"data-element-id\");\n        if (!elementId) {\n          return null;\n        }\n        applySelection([elementId]);\n        return [elementId];\n      },\n      onOverlay: applyMovePreview,\n      onCommit: (commit) => {\n        // Preserve the final preview while the single Rust move command is in flight.\n        applyMovePreview(commit);\n        Promise.resolve(commitMove(commit)).catch((error) => {\n          removeMoveOverlay(svg);\n          removeSnapGuides(svg);\n          onError(error);\n        });\n      },\n      onError,\n    });\n\n    const disposeConnector = bindConnectorPointerSurface(svg, {\n      controller: connectorController,\n      getConnectorKind: () => connectorTool,\n      onOverlay: (preview) => renderConnectorPreview(svg, preview),\n      onCommit: (commit) => {\n        renderConnectorPreview(svg, { ...commit, kind: \"connector-preview\" });\n        Promise.resolve(commitConnector(commit)).catch((error) => {\n          removeConnectorPreview(svg);\n          onError(error);\n        });\n      },\n      onError,\n    });\n\n    disposePointerBinding = () => {\n      disposeConnector();\n      disposeMove();\n      connectorController = null;\n    };\n  };""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n      clearSelection({ notify: false });\n""",
    """      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n      removeConnectorPreview(svg);\n      connectorController = null;\n      clearSelection({ notify: false });\n""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """    setInteractionSettings(settings) {\n""",
    """    setConnectorTool(kind) {\n      const next = kind === null ? null : normalizeConnectorKind(kind);\n      if (connectorController?.isActive) {\n        connectorController.cancel();\n      }\n      removeConnectorPreview(svg);\n      connectorTool = next;\n      if (next === null) {\n        host.removeAttribute(\"data-connector-tool\");\n      } else {\n        host.setAttribute(\"data-connector-tool\", next);\n      }\n      return connectorTool;\n    },\n\n    setInteractionSettings(settings) {\n""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n      clearSelection();\n      host.replaceChildren();\n      svg = null;\n      presentationGeometry = null;\n""",
    """      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n      removeConnectorPreview(svg);\n      connectorController = null;\n      clearSelection();\n      host.replaceChildren();\n      svg = null;\n      presentationGeometry = null;\n""",
)

# The previous snippet occurs in both clear() and dispose(); replace the remaining occurrence too.
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n      clearSelection();\n      host.replaceChildren();\n      svg = null;\n      presentationGeometry = null;\n""",
    """      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n      removeConnectorPreview(svg);\n      connectorController = null;\n      clearSelection();\n      host.replaceChildren();\n      svg = null;\n      presentationGeometry = null;\n""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """    clearMovePreview() {\n      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n    },\n""",
    """    clearMovePreview() {\n      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n    },\n\n    clearConnectorPreview() {\n      removeConnectorPreview(svg);\n    },\n""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """function removeSnapGuides(svg) {\n""",
    """function removeConnectorPreview(svg) {\n  if (!svg) {\n    return;\n  }\n  for (const preview of svg.querySelectorAll(`[${CONNECTOR_PREVIEW_ATTRIBUTE}]`)) {\n    preview.remove();\n  }\n}\n\nfunction renderConnectorPreview(svg, preview) {\n  removeConnectorPreview(svg);\n  if (!svg || preview?.kind !== \"connector-preview\") {\n    return;\n  }\n  const start = preview.startMm;\n  const end = preview.endMm;\n  const element =\n    preview.connectorKind === \"orthogonal\"\n      ? document.createElementNS(SVG_NS, \"polyline\")\n      : document.createElementNS(SVG_NS, \"line\");\n  element.setAttribute(CONNECTOR_PREVIEW_ATTRIBUTE, preview.connectorKind);\n  element.setAttribute(\"pointer-events\", \"none\");\n  element.setAttribute(\"aria-hidden\", \"true\");\n  if (preview.connectorKind === \"orthogonal\") {\n    const points = buildOrthogonalPreviewPoints(start, end);\n    element.setAttribute(\n      \"points\",\n      points.map((point) => `${formatFinite(point.x)},${formatFinite(point.y)}`).join(\" \"),\n    );\n  } else {\n    element.setAttribute(\"x1\", formatFinite(start?.x));\n    element.setAttribute(\"y1\", formatFinite(start?.y));\n    element.setAttribute(\"x2\", formatFinite(end?.x));\n    element.setAttribute(\"y2\", formatFinite(end?.y));\n  }\n  svg.append(element);\n}\n\nfunction removeSnapGuides(svg) {\n""",
)

# --- Styling for tool state and preview ---
append_once(
    "apps/desktop/ui/styles.css",
    "data-ddn-connector-preview",
    """
.page-surface[data-connector-tool] .candidate-svg-document {
  cursor: crosshair;
}

[data-ddn-connector-preview] {
  fill: none;
  stroke: var(--accent);
  stroke-width: 0.7;
  stroke-dasharray: 2 1.5;
  stroke-linecap: round;
  stroke-linejoin: round;
  opacity: 0.92;
}
""",
)

print("Prepared connector drawing slice")
