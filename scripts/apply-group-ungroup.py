from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement marker, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


def replace_all(path: str, old: str, new: str, minimum: int = 1) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count < minimum:
        raise SystemExit(f"{path}: expected at least {minimum} replacement markers, found {count}")
    file.write_text(text.replace(old, new), encoding="utf-8")


def write(path: str, content: str) -> None:
    file = Path(path)
    file.parent.mkdir(parents=True, exist_ok=True)
    file.write_text(content, encoding="utf-8")


# Application boundary: expose existing editor-core grouping semantics.
replace_once(
    "crates/app-core/src/lib.rs",
    '''    /// Create one element through the editor-core semantic command boundary.\n    pub fn create_element(\n''',
    '''    /// Group direct sibling elements through editor-core's structural command.\n    pub fn group_elements(\n        &mut self,\n        group_id: ElementId,\n        element_ids: Vec<ElementId>,\n        name: String,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::GroupElements {\n            group_id,\n            element_ids,\n            name,\n        })\n    }\n\n    /// Ungroup one structural group as one semantic history step.\n    pub fn ungroup(&mut self, group_id: ElementId) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::Ungroup { group_id })\n    }\n\n    /// Create one element through the editor-core semantic command boundary.\n    pub fn create_element(\n''',
)

# Desktop Rust boundary.
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    "mod clipboard;\nmod legacy_import;\n",
    "mod clipboard;\nmod grouping;\nmod legacy_import;\n",
)
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''    snap_elements: Vec<SnapElementDto>,\n    port_targets: Vec<PortTargetDto>,\n''',
    '''    snap_elements: Vec<SnapElementDto>,\n    selection_groups: Vec<SelectionGroupDto>,\n    port_targets: Vec<PortTargetDto>,\n''',
)
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''struct SnapElementDto {\n    element_id: ElementId,\n    bounds_mm: Rect,\n    rotation_deg: f64,\n}\n\n#[derive(Debug, Serialize)]\n#[serde(rename_all = "camelCase")]\nstruct PortTargetDto {\n''',
    '''struct SnapElementDto {\n    element_id: ElementId,\n    bounds_mm: Rect,\n    rotation_deg: f64,\n}\n\n#[derive(Debug, Serialize)]\n#[serde(rename_all = "camelCase")]\nstruct SelectionGroupDto {\n    group_id: ElementId,\n    bounds_mm: Rect,\n    leaf_element_ids: Vec<ElementId>,\n}\n\n#[derive(Debug, Serialize)]\n#[serde(rename_all = "camelCase")]\nstruct PortTargetDto {\n''',
)
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''struct SelectionPropertiesDto {\n    count: usize,\n    primary: Option<ElementPropertiesDto>,\n}\n''',
    '''struct SelectionPropertiesDto {\n    count: usize,\n    primary: Option<ElementPropertiesDto>,\n    can_group: bool,\n    can_ungroup: bool,\n    contains_group: bool,\n}\n''',
)
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''    let snap_elements = plan\n        .items\n        .iter()\n        .map(|item| SnapElementDto {\n            element_id: item.element.id,\n            bounds_mm: item.element.bounds_mm,\n            rotation_deg: item.element.rotation_deg,\n        })\n        .collect();\n    let port_targets = document\n''',
    '''    let selection_groups: Vec<_> = grouping::selection_groups(session.document())\n        .into_iter()\n        .map(|group| SelectionGroupDto {\n            group_id: group.group_id,\n            bounds_mm: group.bounds_mm,\n            leaf_element_ids: group.leaf_element_ids,\n        })\n        .collect();\n    let mut snap_elements: Vec<SnapElementDto> = plan\n        .items\n        .iter()\n        .map(|item| SnapElementDto {\n            element_id: item.element.id,\n            bounds_mm: item.element.bounds_mm,\n            rotation_deg: item.element.rotation_deg,\n        })\n        .collect();\n    snap_elements.extend(selection_groups.iter().map(|group| SnapElementDto {\n        element_id: group.group_id,\n        bounds_mm: group.bounds_mm,\n        rotation_deg: 0.0,\n    }));\n    let port_targets = document\n''',
)
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''        snap_elements,\n        port_targets,\n        svg: rendered.svg,\n''',
    '''        snap_elements,\n        selection_groups,\n        port_targets,\n        svg: rendered.svg,\n''',
)
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''    Ok(SelectionPropertiesDto {\n        count: selected.len(),\n        primary,\n    })\n}\n\n#[tauri::command]\nfn create_basic_element(\n''',
    '''    let grouping_state = grouping::selection_capabilities(\n        session.document(),\n        session.active_page_id(),\n        document.session.active_page_layer_id(),\n        &selected,\n    );\n    Ok(SelectionPropertiesDto {\n        count: selected.len(),\n        primary,\n        can_group: grouping_state.can_group,\n        can_ungroup: grouping_state.can_ungroup,\n        contains_group: grouping_state.contains_group,\n    })\n}\n\n#[tauri::command]\nfn group_selection(\n    state: State<'_, DesktopState>,\n) -> Result<ElementEditResultDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let selected: Vec<_> = document\n        .session\n        .session()\n        .selection()\n        .iter()\n        .copied()\n        .collect();\n    let capabilities = grouping::selection_capabilities(\n        document.session.session().document(),\n        document.session.session().active_page_id(),\n        document.session.active_page_layer_id(),\n        &selected,\n    );\n    if !capabilities.can_group {\n        return Err(CommandError::new(\n            "group_selection_invalid",\n            "Select at least two adjacent top-level elements on the visible, unlocked active layer.",\n        ));\n    }\n\n    let group_id = ElementId::new();\n    document\n        .session\n        .group_elements(group_id, selected, "Group".to_owned())\n        .map_err(|error| CommandError::new("group_selection_failed", error.to_string()))?;\n    document\n        .session\n        .set_selection([group_id])\n        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;\n    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn ungroup_selection(\n    state: State<'_, DesktopState>,\n) -> Result<ElementEditResultDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let selected: Vec<_> = document\n        .session\n        .session()\n        .selection()\n        .iter()\n        .copied()\n        .collect();\n    let page_id = document.session.session().active_page_id();\n    let layer_id = document.session.active_page_layer_id();\n    let children = grouping::selected_group_children(\n        document.session.session().document(),\n        page_id,\n        layer_id,\n        &selected,\n    )\n    .ok_or_else(|| {\n        CommandError::new(\n            "ungroup_selection_invalid",\n            "Select one top-level group on the visible, unlocked active layer.",\n        )\n    })?;\n    let group_id = selected[0];\n\n    document\n        .session\n        .ungroup(group_id)\n        .map_err(|error| CommandError::new("ungroup_selection_failed", error.to_string()))?;\n    document\n        .session\n        .set_selection(children)\n        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;\n    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn create_basic_element(\n''',
)
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''            selection_properties,\n            reorder_selection,\n            copy_selection,\n''',
    '''            selection_properties,\n            group_selection,\n            ungroup_selection,\n            reorder_selection,\n            copy_selection,\n''',
)

# Desktop UI structure and actions.
replace_once(
    "apps/desktop/ui/app.js",
    "import { createZOrderRequest, isZOrderActionEnabled } from './editor-interaction/z-order-actions.mjs';\n",
    "import { createZOrderRequest, isZOrderActionEnabled } from './editor-interaction/z-order-actions.mjs';\nimport { isGroupActionEnabled, isUngroupActionEnabled } from './editor-interaction/group-actions.mjs';\n",
)
replace_once(
    "apps/desktop/ui/app.js",
    '''  bringToFront: document.querySelector('#bring-to-front'),\n  toggleGrid: document.querySelector('#toggle-grid'),\n''',
    '''  bringToFront: document.querySelector('#bring-to-front'),\n  groupSelection: document.querySelector('#group-selection'),\n  ungroupSelection: document.querySelector('#ungroup-selection'),\n  toggleGrid: document.querySelector('#toggle-grid'),\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''const zOrderButtons = [\n  elements.sendToBack,\n  elements.sendBackward,\n  elements.bringForward,\n  elements.bringToFront,\n];\n\nconst actionButtons = [\n''',
    '''const zOrderButtons = [\n  elements.sendToBack,\n  elements.sendBackward,\n  elements.bringForward,\n  elements.bringToFront,\n];\nconst groupingButtons = [elements.groupSelection, elements.ungroupSelection];\n\nconst actionButtons = [\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''  ...zOrderButtons,\n  elements.addRectangle,\n''',
    '''  ...zOrderButtons,\n  ...groupingButtons,\n  elements.addRectangle,\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''  updateZOrderActionState();\n  if (!busy) {\n''',
    '''  updateZOrderActionState();\n  updateGroupingActionState();\n  if (!busy) {\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''function updateClipboardActionState() {\n  const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n  elements.copySelection.disabled = isBusy || selectionCount === 0;\n  elements.duplicateSelection.disabled = isBusy || selectionCount === 0;\n  elements.pasteSelection.disabled = isBusy || !clipboardAvailable;\n}\n''',
    '''function updateClipboardActionState() {\n  const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n  const containsGroup = currentSelectionProperties?.containsGroup === true;\n  elements.copySelection.disabled = isBusy || selectionCount === 0 || containsGroup;\n  elements.duplicateSelection.disabled = isBusy || selectionCount === 0 || containsGroup;\n  elements.pasteSelection.disabled = isBusy || !clipboardAvailable;\n  const groupReason = containsGroup\n    ? 'Structural groups are not copied or duplicated in this slice; ungroup first'\n    : null;\n  if (groupReason) {\n    elements.copySelection.title = groupReason;\n    elements.duplicateSelection.title = groupReason;\n  } else {\n    elements.copySelection.title = 'Copy the current selection (Ctrl/Cmd+C)';\n    elements.duplicateSelection.title = 'Duplicate the current selection (Ctrl/Cmd+D)';\n  }\n}\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''  const enabled = isZOrderActionEnabled({\n    selectionCount,\n    layerVisible: activeLayer?.visible === true,\n    layerLocked: activeLayer?.locked !== false,\n    busy: isBusy,\n  });\n  const reason = isBusy\n''',
    '''  const containsGroup = currentSelectionProperties?.containsGroup === true;\n  const enabled =\n    !containsGroup &&\n    isZOrderActionEnabled({\n      selectionCount,\n      layerVisible: activeLayer?.visible === true,\n      layerLocked: activeLayer?.locked !== false,\n      busy: isBusy,\n    });\n  const reason = isBusy\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''    : selectionCount === 0\n      ? 'Select one or more elements to arrange them'\n      : !activeLayer?.visible\n''',
    '''    : selectionCount === 0\n      ? 'Select one or more elements to arrange them'\n      : containsGroup\n        ? 'Structural groups keep their current z-order in this slice; ungroup before arranging'\n        : !activeLayer?.visible\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''  zOrderButtons.forEach((button, index) => {\n    button.disabled = !enabled;\n    button.title = enabled ? enabledTitles[index] : reason;\n  });\n}\n\nfunction setRecoveryBusy(busy) {\n''',
    '''  zOrderButtons.forEach((button, index) => {\n    button.disabled = !enabled;\n    button.title = enabled ? enabledTitles[index] : reason;\n  });\n}\n\nfunction updateGroupingActionState() {\n  const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n  const canGroup = isGroupActionEnabled({\n    canGroup: currentSelectionProperties?.canGroup === true,\n    busy: isBusy,\n  });\n  const canUngroup = isUngroupActionEnabled({\n    canUngroup: currentSelectionProperties?.canUngroup === true,\n    busy: isBusy,\n  });\n  elements.groupSelection.disabled = !canGroup;\n  elements.ungroupSelection.disabled = !canUngroup;\n  elements.groupSelection.title = canGroup\n    ? 'Group the selected adjacent top-level elements'\n    : isBusy\n      ? 'Finish the current action first'\n      : selectionCount < 2\n        ? 'Select at least two adjacent top-level elements to group them'\n        : 'Grouping requires adjacent top-level elements on the visible, unlocked active layer';\n  elements.ungroupSelection.title = canUngroup\n    ? 'Ungroup the selected structural group'\n    : isBusy\n      ? 'Finish the current action first'\n      : 'Select one top-level group on the visible, unlocked active layer';\n}\n\nfunction setRecoveryBusy(busy) {\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''  updateClipboardActionState();\n  updateZOrderActionState();\n\n  const primary = details?.primary ?? null;\n''',
    '''  updateClipboardActionState();\n  updateZOrderActionState();\n  updateGroupingActionState();\n\n  const primary = details?.primary ?? null;\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''async function applyElementProperties(event) {\n''',
    '''async function groupCurrentSelection() {\n  if (!invoke || currentSelectionProperties?.canGroup !== true) {\n    return;\n  }\n  setBusy(true);\n  try {\n    const result = await invoke('group_selection');\n    renderState(result.state);\n    await refreshPresentation({ preserveSelection: false });\n    const selection = result.selectedElementIds ?? [];\n    svgSurface.setSelection(selection);\n    keyboardSurface?.syncSelectionState(selection);\n    await refreshSelectionProperties();\n    scheduleRecoverySync(250);\n    setStatus('Selection grouped');\n  } catch (error) {\n    setStatus(formatInvokeError(error));\n  } finally {\n    setBusy(false);\n  }\n}\n\nasync function ungroupCurrentSelection() {\n  if (!invoke || currentSelectionProperties?.canUngroup !== true) {\n    return;\n  }\n  setBusy(true);\n  try {\n    const result = await invoke('ungroup_selection');\n    renderState(result.state);\n    await refreshPresentation({ preserveSelection: false });\n    const selection = result.selectedElementIds ?? [];\n    svgSurface.setSelection(selection);\n    keyboardSurface?.syncSelectionState(selection);\n    await refreshSelectionProperties();\n    scheduleRecoverySync(250);\n    setStatus('Group dissolved');\n  } catch (error) {\n    setStatus(formatInvokeError(error));\n  } finally {\n    setBusy(false);\n  }\n}\n\nasync function applyElementProperties(event) {\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''elements.bringToFront.addEventListener('click', () => {\n  void reorderCurrentSelection('bringToFront');\n});\n\nelements.deleteSelection.addEventListener('click', () => {\n''',
    '''elements.bringToFront.addEventListener('click', () => {\n  void reorderCurrentSelection('bringToFront');\n});\n\nelements.groupSelection.addEventListener('click', () => {\n  void groupCurrentSelection();\n});\n\nelements.ungroupSelection.addEventListener('click', () => {\n  void ungroupCurrentSelection();\n});\n\nelements.deleteSelection.addEventListener('click', () => {\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''        const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n        if (\n          ((shortcut === 'delete-selection' ||\n            shortcut === 'copy-selection' ||\n            shortcut === 'duplicate-selection') &&\n            selectionCount === 0) ||\n          (shortcut === 'paste-selection' && !clipboardAvailable)\n        ) {\n''',
    '''        const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n        const containsGroup = currentSelectionProperties?.containsGroup === true;\n        if (\n          ((shortcut === 'delete-selection' ||\n            shortcut === 'copy-selection' ||\n            shortcut === 'duplicate-selection') &&\n            selectionCount === 0) ||\n          ((shortcut === 'copy-selection' || shortcut === 'duplicate-selection') && containsGroup) ||\n          (shortcut === 'paste-selection' && !clipboardAvailable)\n        ) {\n''',
)
replace_once(
    "apps/desktop/ui/app.js",
    '''keyboardSurface = createSvgKeyboardSurface(elements.canvasPage, {\n  getSelection: () => svgSurface.selectedElementIds,\n  setSelection: (elementIds) => svgSurface.setSelection(elementIds),\n  onStatus: setStatus,\n});\n''',
    '''keyboardSurface = createSvgKeyboardSurface(elements.canvasPage, {\n  getSelection: () => svgSurface.selectedElementIds,\n  setSelection: (elementIds) => svgSurface.setSelection(elementIds),\n  resolveElementId: (elementId) => svgSurface.resolveSelectionId(elementId),\n  onStatus: setStatus,\n});\n''',
)

replace_once(
    "apps/desktop/ui/index.html",
    '''            </section>\n            <form id="selection-properties-form" hidden>\n''',
    '''            </section>\n            <section class="arrange-section" aria-labelledby="grouping-title">\n              <div class="arrange-heading">\n                <h3 id="grouping-title">Group</h3>\n                <span>structure</span>\n              </div>\n              <div class="arrange-actions" role="group" aria-label="Grouping actions">\n                <button id="group-selection" type="button" disabled>Group</button>\n                <button id="ungroup-selection" type="button" disabled>Ungroup</button>\n              </div>\n            </section>\n            <form id="selection-properties-form" hidden>\n''',
)

# SVG logical group selection.
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    '''import { snapMoveDelta } from "./editor-interaction/snapping.mjs";\n''',
    '''import { snapMoveDelta } from "./editor-interaction/snapping.mjs";\nimport { createSelectionGroupIndex } from "./editor-interaction/selection-groups.mjs";\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    '''  let selectedElementIds = [];\n  let presentationGeometry = null;\n  let interactionSettings = { ...DEFAULT_INTERACTION_SETTINGS };\n\n  const applySelection = (elementIds, { notify = true } = {}) => {\n    const requested = normalizeElementIds(elementIds);\n''',
    '''  let selectedElementIds = [];\n  let presentationGeometry = null;\n  let selectionGroupIndex = createSelectionGroupIndex();\n  let interactionSettings = { ...DEFAULT_INTERACTION_SETTINGS };\n\n  const applySelection = (elementIds, { notify = true } = {}) => {\n    const requested = normalizeElementIds(\n      normalizeElementIds(elementIds).map((elementId) => selectionGroupIndex.resolveId(elementId)),\n    );\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    '''    const applied = [];\n    if (svg) {\n      for (const elementId of requested) {\n        const element = findRenderableElement(svg, elementId);\n        if (!element) {\n          continue;\n        }\n        element.setAttribute(SELECTED_ATTRIBUTE, "true");\n        applied.push(elementId);\n      }\n    }\n''',
    '''    const applied = [];\n    if (svg) {\n      for (const elementId of requested) {\n        let rendered = false;\n        for (const renderElementId of selectionGroupIndex.renderIds([elementId])) {\n          const element = findRenderableElement(svg, renderElementId);\n          if (!element) {\n            continue;\n          }\n          element.setAttribute(SELECTED_ATTRIBUTE, "true");\n          rendered = true;\n        }\n        if (rendered) {\n          applied.push(elementId);\n        }\n      }\n    }\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    '''    for (const elementId of preview.elementIds) {\n      const source = findRenderableElement(svg, elementId);\n''',
    '''    for (const elementId of selectionGroupIndex.renderIds(preview.elementIds)) {\n      const source = findRenderableElement(svg, elementId);\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    '''      elementIds,\n      elements: presentationGeometry.snapElements,\n''',
    '''      elementIds: selectionGroupIndex.snapIds(elementIds),\n      elements: presentationGeometry.snapElements,\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    '''        const hitElementId =\n          target && svg.contains(target) && !target.closest(`[${MOVE_OVERLAY_ATTRIBUTE}]`)\n            ? target.getAttribute("data-element-id")\n            : null;\n        const resolved = resolveMouseSelection({\n''',
    '''        const rawHitElementId =\n          target && svg.contains(target) && !target.closest(`[${MOVE_OVERLAY_ATTRIBUTE}]`)\n            ? target.getAttribute("data-element-id")\n            : null;\n        const hitElementId = rawHitElementId\n          ? selectionGroupIndex.resolveId(rawHitElementId)\n          : null;\n        const resolved = resolveMouseSelection({\n''',
)
replace_all(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    "      presentationGeometry = null;\n",
    "      presentationGeometry = null;\n      selectionGroupIndex = createSelectionGroupIndex();\n",
    minimum=3,
)
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    '''      host.replaceChildren(svg);\n\n      presentationGeometry = Object.freeze({\n''',
    '''      host.replaceChildren(svg);\n\n      selectionGroupIndex = createSelectionGroupIndex(presentation.selectionGroups ?? []);\n      presentationGeometry = Object.freeze({\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    '''          normalizeSnapElements(presentation.snapElements ?? []).filter((element) =>\n            Boolean(findRenderableElement(svg, element.elementId)),\n          ),\n''',
    '''          normalizeSnapElements(presentation.snapElements ?? []).filter(\n            (element) =>\n              selectionGroupIndex.isGroup(element.elementId) ||\n              Boolean(findRenderableElement(svg, element.elementId)),\n          ),\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    '''    setSelection(elementIds) {\n      return applySelection(elementIds);\n    },\n''',
    '''    resolveSelectionId(elementId) {\n      return selectionGroupIndex.resolveId(elementId);\n    },\n\n    setSelection(elementIds) {\n      return applySelection(elementIds);\n    },\n''',
)

# Keyboard facade collapses rendered group descendants to one logical option.
replace_once(
    "apps/desktop/ui/candidate-svg-keyboard.mjs",
    '''    getSelection,\n    setSelection,\n    onStatus = () => {},\n''',
    '''    getSelection,\n    setSelection,\n    resolveElementId = (elementId) => elementId,\n    onStatus = () => {},\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-keyboard.mjs",
    '''  if (typeof onStatus !== 'function') {\n    throw new TypeError('candidate keyboard status callback must be a function');\n  }\n''',
    '''  if (typeof resolveElementId !== 'function') {\n    throw new TypeError('candidate keyboard element resolver must be a function');\n  }\n  if (typeof onStatus !== 'function') {\n    throw new TypeError('candidate keyboard status callback must be a function');\n  }\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-keyboard.mjs",
    '''    currentElements = listKeyboardElements(host);\n''',
    '''    currentElements = listKeyboardElements(host, resolveElementId);\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-keyboard.mjs",
    '''function listKeyboardElements(host) {\n  const entries = [];\n  for (const element of host.querySelectorAll?.(ELEMENT_SELECTOR) ?? []) {\n''',
    '''function listKeyboardElements(host, resolveElementId) {\n  const entries = [];\n  for (const element of host.querySelectorAll?.(ELEMENT_SELECTOR) ?? []) {\n''',
)
replace_once(
    "apps/desktop/ui/candidate-svg-keyboard.mjs",
    '''    const id = element.getAttribute('data-element-id');\n    if (!id || entries.some((entry) => entry.id === id)) {\n      continue;\n    }\n    entries.push({ id, element });\n''',
    '''    const rawId = element.getAttribute('data-element-id');\n    const id = rawId ? resolveElementId(rawId) : null;\n    if (!id || entries.some((entry) => entry.id === id)) {\n      continue;\n    }\n    entries.push({ id, element });\n''',
)

# New Rust helper with deterministic policy tests.
write(
    "apps/desktop/src-tauri/src/grouping.rs",
    r'''use std::collections::{BTreeMap, BTreeSet};

use next_domain::{Document, Element, ElementId, ElementKind, Layer, LayerId, PageId, Rect, Scene};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SelectionGroupSnapshot {
    pub group_id: ElementId,
    pub bounds_mm: Rect,
    pub leaf_element_ids: Vec<ElementId>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SelectionCapabilities {
    pub can_group: bool,
    pub can_ungroup: bool,
    pub contains_group: bool,
}

pub(crate) fn selection_groups(document: &Document) -> Vec<SelectionGroupSnapshot> {
    let mut groups = Vec::new();
    for layer in document.master_layers.iter().filter(|layer| layer.visible) {
        collect_layer_selection_groups(layer, &mut groups);
    }
    for page in &document.pages {
        for layer in page.layers.iter().filter(|layer| layer.visible) {
            collect_layer_selection_groups(layer, &mut groups);
        }
    }
    groups
}

pub(crate) fn selection_capabilities(
    document: &Document,
    active_page_id: Option<PageId>,
    active_layer_id: Option<LayerId>,
    selected: &[ElementId],
) -> SelectionCapabilities {
    let contains_group = selected.iter().any(|element_id| {
        find_element(document, *element_id)
            .is_some_and(|element| matches!(element.kind, ElementKind::Group { .. }))
    });
    let Some(layer) = active_page_layer(document, active_page_id, active_layer_id) else {
        return SelectionCapabilities {
            contains_group,
            ..SelectionCapabilities::default()
        };
    };
    if !layer.visible || layer.locked {
        return SelectionCapabilities {
            contains_group,
            ..SelectionCapabilities::default()
        };
    }

    let selected_set: BTreeSet<_> = selected.iter().copied().collect();
    let mut positions: Vec<_> = layer
        .scene
        .roots
        .iter()
        .enumerate()
        .filter_map(|(index, element_id)| selected_set.contains(element_id).then_some(index))
        .collect();
    positions.sort_unstable();
    let all_selected_are_roots = positions.len() == selected_set.len();
    let contiguous = positions
        .windows(2)
        .all(|window| window[1] == window[0].saturating_add(1));
    let can_group = selected_set.len() >= 2 && all_selected_are_roots && contiguous;

    let can_ungroup = selected_set.len() == 1
        && positions.len() == 1
        && find_element_in_scene(&layer.scene, *selected_set.iter().next().unwrap())
            .is_some_and(|element| matches!(element.kind, ElementKind::Group { .. }));

    SelectionCapabilities {
        can_group,
        can_ungroup,
        contains_group,
    }
}

pub(crate) fn selected_group_children(
    document: &Document,
    active_page_id: Option<PageId>,
    active_layer_id: Option<LayerId>,
    selected: &[ElementId],
) -> Option<Vec<ElementId>> {
    let capabilities = selection_capabilities(document, active_page_id, active_layer_id, selected);
    if !capabilities.can_ungroup {
        return None;
    }
    let layer = active_page_layer(document, active_page_id, active_layer_id)?;
    let element = find_element_in_scene(&layer.scene, selected[0])?;
    let ElementKind::Group { children } = &element.kind else {
        return None;
    };
    Some(children.clone())
}

fn active_page_layer(
    document: &Document,
    active_page_id: Option<PageId>,
    active_layer_id: Option<LayerId>,
) -> Option<&Layer> {
    let page_id = active_page_id?;
    let layer_id = active_layer_id?;
    document
        .pages
        .iter()
        .find(|page| page.id == page_id)?
        .layers
        .iter()
        .find(|layer| layer.id == layer_id)
}

fn collect_layer_selection_groups(layer: &Layer, output: &mut Vec<SelectionGroupSnapshot>) {
    let elements: BTreeMap<_, _> = layer
        .scene
        .elements
        .iter()
        .map(|element| (element.id, element))
        .collect();
    for root_id in &layer.scene.roots {
        let Some(root) = elements.get(root_id).copied() else {
            continue;
        };
        if !matches!(root.kind, ElementKind::Group { .. }) {
            continue;
        }
        let mut leaves = Vec::new();
        let mut visiting = BTreeSet::new();
        if collect_leaf_ids(*root_id, &elements, &mut visiting, &mut leaves) && !leaves.is_empty() {
            output.push(SelectionGroupSnapshot {
                group_id: *root_id,
                bounds_mm: root.bounds_mm,
                leaf_element_ids: leaves,
            });
        }
    }
}

fn collect_leaf_ids(
    element_id: ElementId,
    elements: &BTreeMap<ElementId, &Element>,
    visiting: &mut BTreeSet<ElementId>,
    output: &mut Vec<ElementId>,
) -> bool {
    if !visiting.insert(element_id) {
        return false;
    }
    let Some(element) = elements.get(&element_id).copied() else {
        visiting.remove(&element_id);
        return false;
    };
    let valid = match &element.kind {
        ElementKind::Group { children } => children
            .iter()
            .all(|child_id| collect_leaf_ids(*child_id, elements, visiting, output)),
        _ => {
            output.push(element_id);
            true
        }
    };
    visiting.remove(&element_id);
    valid
}

fn find_element(document: &Document, element_id: ElementId) -> Option<&Element> {
    document
        .master_layers
        .iter()
        .flat_map(|layer| layer.scene.elements.iter())
        .chain(
            document
                .pages
                .iter()
                .flat_map(|page| page.layers.iter())
                .flat_map(|layer| layer.scene.elements.iter()),
        )
        .find(|element| element.id == element_id)
}

fn find_element_in_scene(scene: &Scene, element_id: ElementId) -> Option<&Element> {
    scene.elements.iter().find(|element| element.id == element_id)
}

#[cfg(test)]
mod tests {
    use next_domain::{
        AnchorSet, ConnectorLabelStyle, DocumentDefaults, DocumentId, LayerId, Page, Rect, Size,
    };

    use super::*;

    fn element(id: ElementId, x: f64, kind: ElementKind) -> Element {
        Element {
            id,
            name: String::new(),
            bounds_mm: Rect {
                x,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind,
            import: None,
        }
    }

    fn fixture(locked: bool) -> (Document, PageId, LayerId, [ElementId; 4]) {
        let ids = [
            ElementId::new(),
            ElementId::new(),
            ElementId::new(),
            ElementId::new(),
        ];
        let nested = ElementId::new();
        let group = ids[0];
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let document = Document {
            id: DocumentId::new(),
            name: String::new(),
            defaults: DocumentDefaults {
                font_family: "Arial".to_owned(),
                font_size_pt: 10.0,
                font_style_bits: 0,
                object_shadows: false,
                auto_line_break: true,
                connector_label_style: ConnectorLabelStyle::Transparent,
            },
            master_layers: Vec::new(),
            pages: vec![Page {
                id: page_id,
                name: String::new(),
                size_mm: Size {
                    width: 210.0,
                    height: 297.0,
                },
                layers: vec![Layer {
                    id: layer_id,
                    name: String::new(),
                    visible: true,
                    locked,
                    draw_color: None,
                    scene: Scene {
                        roots: vec![group, ids[3]],
                        elements: vec![
                            element(
                                group,
                                0.0,
                                ElementKind::Group {
                                    children: vec![nested, ids[2]],
                                },
                            ),
                            element(
                                nested,
                                0.0,
                                ElementKind::Group {
                                    children: vec![ids[1]],
                                },
                            ),
                            element(ids[1], 0.0, ElementKind::Ellipse),
                            element(
                                ids[2],
                                12.0,
                                ElementKind::Rectangle {
                                    corner_radius_mm: 0.0,
                                },
                            ),
                            element(ids[3], 30.0, ElementKind::Ellipse),
                        ],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        };
        (document, page_id, layer_id, ids)
    }

    #[test]
    fn top_level_group_maps_to_rendered_leaf_descendants() {
        let (document, _, _, ids) = fixture(false);
        let groups = selection_groups(&document);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].group_id, ids[0]);
        assert_eq!(groups[0].leaf_element_ids, vec![ids[1], ids[2]]);
    }

    #[test]
    fn capabilities_require_mutable_contiguous_top_level_selection() {
        let (document, page_id, layer_id, ids) = fixture(false);
        let group_only = selection_capabilities(&document, Some(page_id), Some(layer_id), &[ids[0]]);
        assert!(group_only.can_ungroup);
        assert!(group_only.contains_group);

        let adjacent = selection_capabilities(
            &document,
            Some(page_id),
            Some(layer_id),
            &[ids[0], ids[3]],
        );
        assert!(adjacent.can_group);

        let (locked, page_id, layer_id, ids) = fixture(true);
        let blocked = selection_capabilities(
            &locked,
            Some(page_id),
            Some(layer_id),
            &[ids[0], ids[3]],
        );
        assert!(!blocked.can_group);
        assert!(!blocked.can_ungroup);
    }
}
''',
)

# Renderer-neutral frontend grouping policy.
write(
    "apps/desktop/ui/editor-interaction/group-actions.mjs",
    r'''export function isGroupActionEnabled({ canGroup = false, busy = false } = {}) {
  return busy !== true && canGroup === true;
}

export function isUngroupActionEnabled({ canUngroup = false, busy = false } = {}) {
  return busy !== true && canUngroup === true;
}
''',
)
write(
    "apps/desktop/ui/editor-interaction/selection-groups.mjs",
    r'''export function createSelectionGroupIndex(groups = []) {
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
''',
)

# Web test re-exports and deterministic tests.
write(
    "web/editor-interaction/group-actions.mjs",
    "export { isGroupActionEnabled, isUngroupActionEnabled } from '../../apps/desktop/ui/editor-interaction/group-actions.mjs';\n",
)
write(
    "web/editor-interaction/group-actions.test.mjs",
    r'''import test from 'node:test';
import assert from 'node:assert/strict';

import { isGroupActionEnabled, isUngroupActionEnabled } from './group-actions.mjs';

test('group actions follow backend capability and busy state', () => {
  assert.equal(isGroupActionEnabled({ canGroup: true, busy: false }), true);
  assert.equal(isGroupActionEnabled({ canGroup: false, busy: false }), false);
  assert.equal(isGroupActionEnabled({ canGroup: true, busy: true }), false);
  assert.equal(isUngroupActionEnabled({ canUngroup: true, busy: false }), true);
  assert.equal(isUngroupActionEnabled({ canUngroup: false, busy: false }), false);
  assert.equal(isUngroupActionEnabled({ canUngroup: true, busy: true }), false);
});
''',
)
write(
    "web/editor-interaction/selection-groups.mjs",
    "export { createSelectionGroupIndex } from '../../apps/desktop/ui/editor-interaction/selection-groups.mjs';\n",
)
write(
    "web/editor-interaction/selection-groups.test.mjs",
    r'''import test from 'node:test';
import assert from 'node:assert/strict';

import { createSelectionGroupIndex } from './selection-groups.mjs';

test('group descendants resolve to one logical selection and expand for render and snap', () => {
  const index = createSelectionGroupIndex([
    { groupId: 'group-a', leafElementIds: ['leaf-a', 'leaf-b'] },
  ]);
  assert.equal(index.resolveId('leaf-a'), 'group-a');
  assert.equal(index.resolveId('group-a'), 'group-a');
  assert.equal(index.resolveId('free'), 'free');
  assert.equal(index.isGroup('group-a'), true);
  assert.deepEqual(index.renderIds(['group-a']), ['leaf-a', 'leaf-b']);
  assert.deepEqual(index.renderIds(['leaf-a', 'free']), ['leaf-a', 'leaf-b', 'free']);
  assert.deepEqual(index.snapIds(['group-a']), ['group-a', 'leaf-a', 'leaf-b']);
});

test('selection group index rejects ambiguous descendant ownership', () => {
  assert.throws(
    () =>
      createSelectionGroupIndex([
        { groupId: 'group-a', leafElementIds: ['leaf'] },
        { groupId: 'group-b', leafElementIds: ['leaf'] },
      ]),
    /belongs to more than one selection group/,
  );
});
''',
)

# Application persistence/history regression test.
write(
    "crates/app-core/tests/grouping_application.rs",
    r'''use app_core::ApplicationSession;
use ddnx::PackageLimits;
use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene, Size,
};

fn rectangle(id: ElementId, x: f64) -> Element {
    Element {
        id,
        name: "Rectangle".to_owned(),
        bounds_mm: Rect {
            x,
            y: 20.0,
            width: 20.0,
            height: 10.0,
        },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::Rectangle {
            corner_radius_mm: 0.0,
        },
        import: None,
    }
}

fn fixture() -> (NextArtifact, [ElementId; 3]) {
    let ids = [ElementId::new(), ElementId::new(), ElementId::new()];
    let document = Document {
        id: DocumentId::new(),
        name: "Grouping application test".to_owned(),
        defaults: DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Transparent,
        },
        master_layers: Vec::new(),
        pages: vec![Page {
            id: PageId::new(),
            name: "Page 1".to_owned(),
            size_mm: Size {
                width: 210.0,
                height: 297.0,
            },
            layers: vec![Layer {
                id: LayerId::new(),
                name: "Layer 1".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: ids.to_vec(),
                    elements: vec![
                        rectangle(ids[0], 10.0),
                        rectangle(ids[1], 35.0),
                        rectangle(ids[2], 70.0),
                    ],
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (NextArtifact::document(document), ids)
}

fn roots(app: &ApplicationSession) -> Vec<ElementId> {
    app.session().document().pages[0].layers[0].scene.roots.clone()
}

#[test]
fn grouping_round_trips_through_application_history_and_ddnx() {
    let (artifact, ids) = fixture();
    let mut app = ApplicationSession::from_artifact(artifact).unwrap();
    let initial_history = app.session().current_history_state();
    let group_id = ElementId::new();

    assert!(
        app.group_elements(group_id, vec![ids[0], ids[1]], "Pair".to_owned())
            .unwrap()
    );
    assert_eq!(roots(&app), vec![group_id, ids[2]]);
    let group = app.session().document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == group_id)
        .unwrap();
    let ElementKind::Group { children } = &group.kind else {
        panic!("expected structural group")
    };
    assert_eq!(children, &vec![ids[0], ids[1]]);
    let grouped_history = app.session().current_history_state();
    assert_ne!(grouped_history, initial_history);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened =
        ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(roots(&reopened), vec![group_id, ids[2]]);

    assert!(app.undo().unwrap());
    assert_eq!(roots(&app), ids.to_vec());
    assert_eq!(app.session().current_history_state(), initial_history);
    assert!(app.redo().unwrap());
    assert_eq!(roots(&app), vec![group_id, ids[2]]);
    assert_eq!(app.session().current_history_state(), grouped_history);

    assert!(app.ungroup(group_id).unwrap());
    assert_eq!(roots(&app), ids.to_vec());
    let ungrouped_history = app.session().current_history_state();
    assert_ne!(ungrouped_history, initial_history);
    assert!(app.undo().unwrap());
    assert_eq!(roots(&app), vec![group_id, ids[2]]);
    assert!(app.redo().unwrap());
    assert_eq!(roots(&app), ids.to_vec());
}
''',
)

print("group/ungroup patch applied")
