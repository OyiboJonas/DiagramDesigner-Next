from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one match in {path}: {old[:100]!r}, got {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


# Tauri Rust boundary
lib = Path("apps/desktop/src-tauri/src/lib.rs")
replace_once(
    lib,
    """use app_core::{\n    ApplicationSession, ConnectorEndpointSide as AppConnectorEndpointSide,""",
    """use app_core::{\n    ApplicationSession, ArrangeOperation as AppArrangeOperation,\n    ConnectorEndpointSide as AppConnectorEndpointSide,""",
)
replace_once(
    lib,
    """struct ReorderSelectionRequest {\n    operation: ZOrderOperationRequest,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\nenum BasicElementKind""",
    """struct ReorderSelectionRequest {\n    operation: ZOrderOperationRequest,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nenum ArrangeOperationRequest {\n    AlignLeft,\n    AlignHorizontalCenter,\n    AlignRight,\n    AlignTop,\n    AlignVerticalCenter,\n    AlignBottom,\n    DistributeHorizontal,\n    DistributeVertical,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct ArrangeSelectionRequest {\n    operation: ArrangeOperationRequest,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\nenum BasicElementKind""",
)

arrange_command = r'''
#[tauri::command]
fn arrange_selection(
    request: ArrangeSelectionRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let selected: Vec<_> = document
        .session
        .session()
        .selection()
        .iter()
        .copied()
        .collect();

    {
        let session = document.session.session();
        let page_id = session.active_page_id().ok_or_else(|| {
            CommandError::new(
                "layout_no_active_page",
                "Choose an active page before aligning or distributing elements.",
            )
        })?;
        let layer_id = document.session.active_page_layer_id().ok_or_else(|| {
            CommandError::new(
                "layout_no_active_layer",
                "Choose a page-local layer before aligning or distributing elements.",
            )
        })?;
        let page = session
            .document()
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .ok_or_else(|| {
                CommandError::new("layout_page_missing", "The active page no longer exists.")
            })?;
        let layer = page
            .layers
            .iter()
            .find(|layer| layer.id == layer_id)
            .ok_or_else(|| {
                CommandError::new("layout_layer_missing", "The active layer no longer exists.")
            })?;
        if !layer.visible {
            return Err(CommandError::new(
                "layout_layer_hidden",
                "Elements can be aligned or distributed only on a visible active layer.",
            ));
        }
        if layer.locked {
            return Err(CommandError::new(
                "layout_layer_locked",
                "Unlock the active layer before aligning or distributing elements.",
            ));
        }
        if selected.iter().any(|element_id| {
            !layer
                .scene
                .elements
                .iter()
                .any(|element| element.id == *element_id)
        }) {
            return Err(CommandError::new(
                "layout_not_on_active_layer",
                "Every selected element must belong to the active layer.",
            ));
        }
    }

    let operation = match request.operation {
        ArrangeOperationRequest::AlignLeft => AppArrangeOperation::AlignLeft,
        ArrangeOperationRequest::AlignHorizontalCenter => AppArrangeOperation::AlignHorizontalCenter,
        ArrangeOperationRequest::AlignRight => AppArrangeOperation::AlignRight,
        ArrangeOperationRequest::AlignTop => AppArrangeOperation::AlignTop,
        ArrangeOperationRequest::AlignVerticalCenter => AppArrangeOperation::AlignVerticalCenter,
        ArrangeOperationRequest::AlignBottom => AppArrangeOperation::AlignBottom,
        ArrangeOperationRequest::DistributeHorizontal => AppArrangeOperation::DistributeHorizontal,
        ArrangeOperationRequest::DistributeVertical => AppArrangeOperation::DistributeVertical,
    };
    document
        .session
        .arrange_elements(selected, operation)
        .map_err(|error| CommandError::new("layout_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

'''
replace_once(
    lib,
    """#[tauri::command]\nfn copy_selection(state: State<'_, DesktopState>) -> Result<ClipboardCopyDto, CommandError> {""",
    arrange_command + "#[tauri::command]\nfn copy_selection(state: State<'_, DesktopState>) -> Result<ClipboardCopyDto, CommandError> {",
)
replace_once(
    lib,
    """            ungroup_selection,\n            reorder_selection,\n            copy_selection,""",
    """            ungroup_selection,\n            reorder_selection,\n            arrange_selection,\n            copy_selection,""",
)

# Tauri ACL / command registry
build = Path("apps/desktop/src-tauri/build.rs")
replace_once(
    build,
    '            "reorder_selection",\n            "copy_selection",',
    '            "reorder_selection",\n            "arrange_selection",\n            "copy_selection",',
)
permissions = Path("apps/desktop/src-tauri/permissions/editor.toml")
replace_once(
    permissions,
    '''[[permission]]\nidentifier = "allow-copy-selection"\ndescription = "Allows the main editor window to invoke the copy_selection application command."\ncommands.allow = ["copy_selection"]'''.replace('\\n', '\n'),
    '''[[permission]]\nidentifier = "allow-arrange-selection"\ndescription = "Allows the main editor window to invoke the arrange_selection application command."\ncommands.allow = ["arrange_selection"]\n\n[[permission]]\nidentifier = "allow-copy-selection"\ndescription = "Allows the main editor window to invoke the copy_selection application command."\ncommands.allow = ["copy_selection"]'''.replace('\\n', '\n'),
)
capability = Path("apps/desktop/src-tauri/capabilities/main-editor.json")
replace_once(
    capability,
    '    "allow-reorder-selection",\n    "allow-copy-selection",',
    '    "allow-reorder-selection",\n    "allow-arrange-selection",\n    "allow-copy-selection",',
)

# Frontend helper contract
helper = Path("apps/desktop/ui/editor-interaction/arrange-actions.mjs")
helper.parent.mkdir(parents=True, exist_ok=True)
helper.write_text(r'''const ARRANGE_MINIMUMS = new Map([
  ['alignLeft', 2],
  ['alignHorizontalCenter', 2],
  ['alignRight', 2],
  ['alignTop', 2],
  ['alignVerticalCenter', 2],
  ['alignBottom', 2],
  ['distributeHorizontal', 3],
  ['distributeVertical', 3],
]);

export function arrangeMinimumSelection(operation) {
  const minimum = ARRANGE_MINIMUMS.get(operation);
  if (minimum === undefined) {
    throw new TypeError(`Unsupported align/distribute operation: ${String(operation)}`);
  }
  return minimum;
}

export function createArrangeRequest(operation) {
  arrangeMinimumSelection(operation);
  return { operation };
}

export function isArrangeActionEnabled({
  operation,
  selectionCount = 0,
  layerVisible = false,
  layerLocked = true,
  busy = false,
} = {}) {
  const minimum = arrangeMinimumSelection(operation);
  return (
    !busy &&
    Number(selectionCount) >= minimum &&
    layerVisible === true &&
    layerLocked !== true
  );
}
''', encoding="utf-8")

web_helper = Path("web/editor-interaction/arrange-actions.mjs")
web_helper.parent.mkdir(parents=True, exist_ok=True)
web_helper.write_text(r'''export {
  arrangeMinimumSelection,
  createArrangeRequest,
  isArrangeActionEnabled,
} from '../../apps/desktop/ui/editor-interaction/arrange-actions.mjs';
''', encoding="utf-8")

web_test = Path("web/editor-interaction/arrange-actions.test.mjs")
web_test.write_text(r'''import test from 'node:test';
import assert from 'node:assert/strict';

import {
  arrangeMinimumSelection,
  createArrangeRequest,
  isArrangeActionEnabled,
} from './arrange-actions.mjs';

const alignOperations = [
  'alignLeft',
  'alignHorizontalCenter',
  'alignRight',
  'alignTop',
  'alignVerticalCenter',
  'alignBottom',
];
const distributeOperations = ['distributeHorizontal', 'distributeVertical'];

test('all align and distribute operations map to the desktop request contract', () => {
  for (const operation of [...alignOperations, ...distributeOperations]) {
    assert.deepEqual(createArrangeRequest(operation), { operation });
  }
  assert.throws(() => createArrangeRequest('alignMagic'), /Unsupported align\/distribute operation/);
});

test('alignment requires two logical selection items', () => {
  for (const operation of alignOperations) {
    assert.equal(arrangeMinimumSelection(operation), 2);
    assert.equal(isArrangeActionEnabled({
      operation,
      selectionCount: 2,
      layerVisible: true,
      layerLocked: false,
      busy: false,
    }), true);
    assert.equal(isArrangeActionEnabled({
      operation,
      selectionCount: 1,
      layerVisible: true,
      layerLocked: false,
      busy: false,
    }), false);
  }
});

test('distribution requires three logical selection items', () => {
  for (const operation of distributeOperations) {
    assert.equal(arrangeMinimumSelection(operation), 3);
    assert.equal(isArrangeActionEnabled({
      operation,
      selectionCount: 3,
      layerVisible: true,
      layerLocked: false,
      busy: false,
    }), true);
    assert.equal(isArrangeActionEnabled({
      operation,
      selectionCount: 2,
      layerVisible: true,
      layerLocked: false,
      busy: false,
    }), false);
  }
});

test('hidden locked and busy layers disable every arrange operation', () => {
  for (const operation of [...alignOperations, ...distributeOperations]) {
    const selectionCount = arrangeMinimumSelection(operation);
    assert.equal(isArrangeActionEnabled({ operation, selectionCount, layerVisible: false, layerLocked: false }), false);
    assert.equal(isArrangeActionEnabled({ operation, selectionCount, layerVisible: true, layerLocked: true }), false);
    assert.equal(isArrangeActionEnabled({ operation, selectionCount, layerVisible: true, layerLocked: false, busy: true }), false);
  }
});

test('arrange eligibility depends on logical selection count, not group special cases', () => {
  assert.equal(isArrangeActionEnabled({
    operation: 'alignLeft',
    selectionCount: 2,
    layerVisible: true,
    layerLocked: false,
    busy: false,
    containsGroup: true,
  }), true);
});
''', encoding="utf-8")

# Desktop HTML controls
index = Path("apps/desktop/ui/index.html")
replace_once(
    index,
    '''            <section class="arrange-section" aria-labelledby="grouping-title">''',
    '''            <section class="arrange-section" aria-labelledby="align-distribute-title">
              <div class="arrange-heading">
                <h3 id="align-distribute-title">Align &amp; distribute</h3>
                <span>layout</span>
              </div>
              <div class="arrange-actions" role="group" aria-label="Alignment and distribution actions">
                <button id="align-left" type="button" disabled>Left</button>
                <button id="align-horizontal-center" type="button" disabled>H center</button>
                <button id="align-right" type="button" disabled>Right</button>
                <button id="align-top" type="button" disabled>Top</button>
                <button id="align-vertical-center" type="button" disabled>V center</button>
                <button id="align-bottom" type="button" disabled>Bottom</button>
                <button id="distribute-horizontal" type="button" disabled>Distribute H</button>
                <button id="distribute-vertical" type="button" disabled>Distribute V</button>
              </div>
            </section>
            <section class="arrange-section" aria-labelledby="grouping-title">''',
)

# Desktop app wiring
app = Path("apps/desktop/ui/app.js")
replace_once(
    app,
    """import { createZOrderRequest, isZOrderActionEnabled } from './editor-interaction/z-order-actions.mjs';\n""",
    """import { createZOrderRequest, isZOrderActionEnabled } from './editor-interaction/z-order-actions.mjs';\nimport {\n  arrangeMinimumSelection,\n  createArrangeRequest,\n  isArrangeActionEnabled,\n} from './editor-interaction/arrange-actions.mjs';\n""",
)
replace_once(
    app,
    """  bringToFront: document.querySelector('#bring-to-front'),\n  groupSelection:""",
    """  bringToFront: document.querySelector('#bring-to-front'),\n  alignLeft: document.querySelector('#align-left'),\n  alignHorizontalCenter: document.querySelector('#align-horizontal-center'),\n  alignRight: document.querySelector('#align-right'),\n  alignTop: document.querySelector('#align-top'),\n  alignVerticalCenter: document.querySelector('#align-vertical-center'),\n  alignBottom: document.querySelector('#align-bottom'),\n  distributeHorizontal: document.querySelector('#distribute-horizontal'),\n  distributeVertical: document.querySelector('#distribute-vertical'),\n  groupSelection:""",
)
replace_once(
    app,
    """const groupingButtons = [elements.groupSelection, elements.ungroupSelection];\n\nconst actionButtons = [""",
    """const arrangeActionEntries = [\n  [elements.alignLeft, 'alignLeft', 'Align left edges'],\n  [elements.alignHorizontalCenter, 'alignHorizontalCenter', 'Align horizontal centers'],\n  [elements.alignRight, 'alignRight', 'Align right edges'],\n  [elements.alignTop, 'alignTop', 'Align top edges'],\n  [elements.alignVerticalCenter, 'alignVerticalCenter', 'Align vertical centers'],\n  [elements.alignBottom, 'alignBottom', 'Align bottom edges'],\n  [elements.distributeHorizontal, 'distributeHorizontal', 'Distribute horizontally'],\n  [elements.distributeVertical, 'distributeVertical', 'Distribute vertically'],\n];\nconst arrangeButtons = arrangeActionEntries.map(([button]) => button);\nconst groupingButtons = [elements.groupSelection, elements.ungroupSelection];\n\nconst actionButtons = [""",
)
replace_once(
    app,
    """  ...zOrderButtons,\n  ...groupingButtons,""",
    """  ...zOrderButtons,\n  ...arrangeButtons,\n  ...groupingButtons,""",
)
replace_once(
    app,
    """  updateZOrderActionState();\n  updateGroupingActionState();\n  if (!busy) {""",
    """  updateZOrderActionState();\n  updateArrangeActionState();\n  updateGroupingActionState();\n  if (!busy) {""",
)

arrange_state = r'''
function updateArrangeActionState() {
  const selectionCount = Number(currentSelectionProperties?.count ?? 0);
  const activeLayer = activeLayerForZOrder();
  for (const [button, operation, enabledTitle] of arrangeActionEntries) {
    const minimum = arrangeMinimumSelection(operation);
    const enabled = isArrangeActionEnabled({
      operation,
      selectionCount,
      layerVisible: activeLayer?.visible === true,
      layerLocked: activeLayer?.locked !== false,
      busy: isBusy,
    });
    const reason = isBusy
      ? 'Finish the current action first'
      : selectionCount < minimum
        ? `${minimum} or more selected logical objects required`
        : !activeLayer?.visible
          ? 'Show the active layer before aligning or distributing elements'
          : activeLayer?.locked
            ? 'Unlock the active layer before aligning or distributing elements'
            : enabledTitle;
    button.disabled = !enabled;
    button.title = enabled ? enabledTitle : reason;
  }
}

'''
replace_once(
    app,
    """function updateGroupingActionState() {""",
    arrange_state + "function updateGroupingActionState() {",
)
replace_once(
    app,
    """  updateZOrderActionState();\n}\n\nfunction renderNavigation""",
    """  updateZOrderActionState();\n  updateArrangeActionState();\n}\n\nfunction renderNavigation""",
)
replace_once(
    app,
    """  updateClipboardActionState();\n  updateZOrderActionState();\n  updateGroupingActionState();""",
    """  updateClipboardActionState();\n  updateZOrderActionState();\n  updateArrangeActionState();\n  updateGroupingActionState();""",
)

arrange_run = r'''
async function arrangeCurrentSelection(operation) {
  const minimum = arrangeMinimumSelection(operation);
  if (!invoke || Number(currentSelectionProperties?.count ?? 0) < minimum) {
    return;
  }
  const labels = {
    alignLeft: 'Selection aligned left',
    alignHorizontalCenter: 'Selection centers aligned horizontally',
    alignRight: 'Selection aligned right',
    alignTop: 'Selection aligned top',
    alignVerticalCenter: 'Selection centers aligned vertically',
    alignBottom: 'Selection aligned bottom',
    distributeHorizontal: 'Selection distributed horizontally',
    distributeVertical: 'Selection distributed vertically',
  };
  setBusy(true);
  try {
    const result = await invoke('arrange_selection', { request: createArrangeRequest(operation) });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: true });
    const selection = result.selectedElementIds ?? svgSurface.selectedElementIds;
    svgSurface.setSelection(selection);
    keyboardSurface?.syncSelectionState(selection);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus(labels[operation]);
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

'''
replace_once(
    app,
    """async function groupCurrentSelection() {""",
    arrange_run + "async function groupCurrentSelection() {",
)
replace_once(
    app,
    """elements.bringToFront.addEventListener('click', () => {\n  void reorderCurrentSelection('bringToFront');\n});\n\nelements.groupSelection""",
    """elements.bringToFront.addEventListener('click', () => {\n  void reorderCurrentSelection('bringToFront');\n});\n\nfor (const [button, operation] of arrangeActionEntries) {\n  button.addEventListener('click', () => {\n    void arrangeCurrentSelection(operation);\n  });\n}\n\nelements.groupSelection""",
)
