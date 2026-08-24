from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"Anchor not found in {path}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """    if !request.visible {\n        document.session.clear_selection();\n    }\n""",
    """    if !request.visible || request.locked {\n        document.session.clear_selection();\n    }\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """  elements.deleteLayer.disabled =\n    isBusy || !activePage || activePage.layers.length <= 1 || !activeLayer || activeLayer.locked;\n  elements.deleteLayer.title = activeLayer?.locked\n    ? 'Unlock the layer before deleting it'\n    : 'Delete the active layer';\n  elements.applyPageProperties.disabled = isBusy || !activePage;\n  elements.applyLayerProperties.disabled = isBusy || !activeLayer;\n""",
    """  elements.deleteLayer.disabled =\n    isBusy || !activePage || activePage.layers.length <= 1 || !activeLayer || activeLayer.locked;\n  elements.deleteLayer.title = activeLayer?.locked\n    ? 'Unlock the layer before deleting it'\n    : 'Delete the active layer';\n  const layerEditable = Boolean(activeLayer?.visible && !activeLayer?.locked);\n  elements.addRectangle.disabled = isBusy || !layerEditable;\n  elements.addText.disabled = isBusy || !layerEditable;\n  elements.addRectangle.title = layerEditable\n    ? 'Create a rectangle on the active layer'\n    : 'Choose a visible, unlocked layer to create elements';\n  elements.addText.title = layerEditable\n    ? 'Create a text box on the active layer'\n    : 'Choose a visible, unlocked layer to create elements';\n  elements.applyPageProperties.disabled = isBusy || !activePage;\n  elements.applyLayerProperties.disabled = isBusy || !activeLayer;\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """    { preserveSelection: elements.layerVisible.checked },\n""",
    """    { preserveSelection: elements.layerVisible.checked && !elements.layerLocked.checked },\n""",
)

print("Applied page/layer editable-state fix")
