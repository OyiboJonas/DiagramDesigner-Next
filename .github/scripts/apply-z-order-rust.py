from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one match in {path}, found {count}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


app = "apps/desktop/ui/app.js"
replace_once(
    app,
    "import { isTextEditingTarget, resolveApplicationShortcut } from './editor-interaction/app-shortcuts.mjs';\n",
    "import { isTextEditingTarget, resolveApplicationShortcut } from './editor-interaction/app-shortcuts.mjs';\n"
    "import { createZOrderRequest, isZOrderActionEnabled } from './editor-interaction/z-order-actions.mjs';\n",
)
replace_once(
    app,
    "  duplicateSelection: document.querySelector('#duplicate-selection'),\n  toggleGrid: document.querySelector('#toggle-grid'),\n",
    "  duplicateSelection: document.querySelector('#duplicate-selection'),\n"
    "  sendToBack: document.querySelector('#send-to-back'),\n"
    "  sendBackward: document.querySelector('#send-backward'),\n"
    "  bringForward: document.querySelector('#bring-forward'),\n"
    "  bringToFront: document.querySelector('#bring-to-front'),\n"
    "  toggleGrid: document.querySelector('#toggle-grid'),\n",
)
replace_once(
    app,
    "const actionButtons = [\n",
    "const zOrderButtons = [\n"
    "  elements.sendToBack,\n"
    "  elements.sendBackward,\n"
    "  elements.bringForward,\n"
    "  elements.bringToFront,\n"
    "];\n\n"
    "const actionButtons = [\n",
)
replace_once(
    app,
    "  elements.duplicateSelection,\n  elements.addRectangle,\n",
    "  elements.duplicateSelection,\n"
    "  ...zOrderButtons,\n"
    "  elements.addRectangle,\n",
)
replace_once(
    app,
    "  elements.pageSelect.disabled = busy;\n  elements.layerSelect.disabled = busy;\n  if (!busy) {\n",
    "  elements.pageSelect.disabled = busy;\n"
    "  elements.layerSelect.disabled = busy;\n"
    "  updateZOrderActionState();\n"
    "  if (!busy) {\n",
)
replace_once(
    app,
    "function setRecoveryBusy(busy) {\n",
    "function activeLayerForZOrder() {\n"
    "  const pages = currentNavigation?.pages ?? [];\n"
    "  const activePage = pages.find((page) => page.pageId === currentNavigation?.activePageId) ?? null;\n"
    "  return activePage?.layers.find((layer) => layer.layerId === currentNavigation?.activeLayerId) ?? null;\n"
    "}\n\n"
    "function updateZOrderActionState() {\n"
    "  const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n"
    "  const activeLayer = activeLayerForZOrder();\n"
    "  const enabled = isZOrderActionEnabled({\n"
    "    selectionCount,\n"
    "    layerVisible: activeLayer?.visible === true,\n"
    "    layerLocked: activeLayer?.locked !== false,\n"
    "    busy: isBusy,\n"
    "  });\n"
    "  const reason = isBusy\n"
    "    ? 'Finish the current action first'\n"
    "    : selectionCount === 0\n"
    "      ? 'Select one or more elements to arrange them'\n"
    "      : !activeLayer?.visible\n"
    "        ? 'Show the active layer before arranging elements'\n"
    "        : activeLayer?.locked\n"
    "          ? 'Unlock the active layer before arranging elements'\n"
    "          : 'Arrange the current selection';\n"
    "  const enabledTitles = [\n"
    "    'Send the selection behind all other elements',\n"
    "    'Move the selection one step backward',\n"
    "    'Move the selection one step forward',\n"
    "    'Bring the selection in front of all other elements',\n"
    "  ];\n"
    "  zOrderButtons.forEach((button, index) => {\n"
    "    button.disabled = !enabled;\n"
    "    button.title = enabled ? enabledTitles[index] : reason;\n"
    "  });\n"
    "}\n\n"
    "function setRecoveryBusy(busy) {\n",
)
replace_once(
    app,
    "  elements.applyLayerProperties.disabled = isBusy || !activeLayer;\n}\n\nfunction renderNavigation(navigation) {\n",
    "  elements.applyLayerProperties.disabled = isBusy || !activeLayer;\n"
    "  updateZOrderActionState();\n"
    "}\n\n"
    "function renderNavigation(navigation) {\n",
)
replace_once(
    app,
    "  elements.deleteSelection.disabled = count === 0;\n  updateClipboardActionState();\n\n  const primary = details?.primary ?? null;\n",
    "  elements.deleteSelection.disabled = count === 0;\n"
    "  updateClipboardActionState();\n"
    "  updateZOrderActionState();\n\n"
    "  const primary = details?.primary ?? null;\n",
)
replace_once(
    app,
    "async function applyElementProperties(event) {\n",
    "async function reorderCurrentSelection(operation) {\n"
    "  if (!invoke || Number(currentSelectionProperties?.count ?? 0) === 0) {\n"
    "    return;\n"
    "  }\n"
    "  const labels = {\n"
    "    sendToBack: 'Selection sent to back',\n"
    "    sendBackward: 'Selection moved backward',\n"
    "    bringForward: 'Selection moved forward',\n"
    "    bringToFront: 'Selection brought to front',\n"
    "  };\n"
    "  setBusy(true);\n"
    "  try {\n"
    "    const result = await invoke('reorder_selection', { request: createZOrderRequest(operation) });\n"
    "    renderState(result.state);\n"
    "    await refreshPresentation({ preserveSelection: true });\n"
    "    const selection = result.selectedElementIds ?? svgSurface.selectedElementIds;\n"
    "    svgSurface.setSelection(selection);\n"
    "    keyboardSurface?.syncSelectionState(selection);\n"
    "    await refreshSelectionProperties();\n"
    "    scheduleRecoverySync(250);\n"
    "    setStatus(labels[operation]);\n"
    "  } catch (error) {\n"
    "    setStatus(formatInvokeError(error));\n"
    "  } finally {\n"
    "    setBusy(false);\n"
    "  }\n"
    "}\n\n"
    "async function applyElementProperties(event) {\n",
)
replace_once(
    app,
    "elements.duplicateSelection.addEventListener('click', () => {\n  void duplicateCurrentSelection();\n});\n\nelements.deleteSelection.addEventListener('click', () => {\n",
    "elements.duplicateSelection.addEventListener('click', () => {\n"
    "  void duplicateCurrentSelection();\n"
    "});\n\n"
    "elements.sendToBack.addEventListener('click', () => {\n"
    "  void reorderCurrentSelection('sendToBack');\n"
    "});\n\n"
    "elements.sendBackward.addEventListener('click', () => {\n"
    "  void reorderCurrentSelection('sendBackward');\n"
    "});\n\n"
    "elements.bringForward.addEventListener('click', () => {\n"
    "  void reorderCurrentSelection('bringForward');\n"
    "});\n\n"
    "elements.bringToFront.addEventListener('click', () => {\n"
    "  void reorderCurrentSelection('bringToFront');\n"
    "});\n\n"
    "elements.deleteSelection.addEventListener('click', () => {\n",
)

index = "apps/desktop/ui/index.html"
replace_once(
    index,
    "            </div>\n            <form id=\"selection-properties-form\" hidden>\n",
    "            </div>\n"
    "            <section class=\"arrange-section\" aria-labelledby=\"arrange-title\">\n"
    "              <div class=\"arrange-heading\">\n"
    "                <h3 id=\"arrange-title\">Arrange</h3>\n"
    "                <span>z-order</span>\n"
    "              </div>\n"
    "              <div class=\"arrange-actions\" role=\"group\" aria-label=\"Z-order actions\">\n"
    "                <button id=\"send-to-back\" type=\"button\" disabled>To back</button>\n"
    "                <button id=\"send-backward\" type=\"button\" disabled>Backward</button>\n"
    "                <button id=\"bring-forward\" type=\"button\" disabled>Forward</button>\n"
    "                <button id=\"bring-to-front\" type=\"button\" disabled>To front</button>\n"
    "              </div>\n"
    "            </section>\n"
    "            <form id=\"selection-properties-form\" hidden>\n",
)

styles = "apps/desktop/ui/styles.css"
path = Path(styles)
text = path.read_text(encoding="utf-8")
marker = "\n/* Z-order arrange controls */\n"
if marker in text:
    raise SystemExit("z-order styles already present")
text += (
    marker
    + ".arrange-section {\n"
    + "  margin-top: 12px;\n"
    + "  padding: 9px;\n"
    + "  border: 1px solid var(--border);\n"
    + "  border-radius: 9px;\n"
    + "  background: var(--surface-subtle);\n"
    + "}\n\n"
    + ".arrange-heading {\n"
    + "  display: flex;\n"
    + "  align-items: center;\n"
    + "  justify-content: space-between;\n"
    + "  gap: 10px;\n"
    + "}\n\n"
    + ".arrange-heading h3 {\n"
    + "  margin: 0;\n"
    + "  font-size: 0.82rem;\n"
    + "}\n\n"
    + ".arrange-heading span {\n"
    + "  color: var(--muted);\n"
    + "  font-size: 0.72rem;\n"
    + "}\n\n"
    + ".arrange-actions {\n"
    + "  display: grid;\n"
    + "  grid-template-columns: repeat(2, minmax(0, 1fr));\n"
    + "  gap: 7px;\n"
    + "  margin-top: 8px;\n"
    + "}\n\n"
    + ".arrange-actions button {\n"
    + "  width: 100%;\n"
    + "  padding-inline: 8px;\n"
    + "}\n"
)
path.write_text(text, encoding="utf-8")
