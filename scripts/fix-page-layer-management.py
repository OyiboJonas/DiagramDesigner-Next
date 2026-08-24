from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"Anchor not found in {path}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "crates/app-core/src/lib.rs",
    """        app.set_active_page_layer(second_page, extra_layer).unwrap();\n        let after_layer_create = app.session().current_history_state();\n        assert_eq!(app.active_page_layer_id(), Some(extra_layer));\n""",
    """        app.set_active_page_layer(second_page, extra_layer).unwrap();\n        assert_eq!(app.active_page_layer_id(), Some(extra_layer));\n""",
)

replace_once(
    "crates/app-core/src/lib.rs",
    """        assert!(!layer.visible);\n        assert!(layer.locked);\n\n        assert!(app.delete_page_layer(second_page, extra_layer).unwrap());\n""",
    """        assert!(!layer.visible);\n        assert!(layer.locked);\n\n        assert!(\n            app.set_page_layer_properties(\n                second_page,\n                extra_layer,\n                \"Annotations\".to_owned(),\n                true,\n                false,\n                None,\n            )\n            .unwrap()\n        );\n        assert!(app.delete_page_layer(second_page, extra_layer).unwrap());\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """function updateStructureDisabledState() {\n  const pages = currentNavigation?.pages ?? [];\n  const activePage = pages.find((page) => page.pageId === currentNavigation?.activePageId) ?? null;\n  elements.deletePage.disabled = isBusy || pages.length <= 1 || !activePage;\n  elements.addLayer.disabled = isBusy || !activePage;\n  elements.deleteLayer.disabled = isBusy || !activePage || activePage.layers.length <= 1;\n  elements.applyPageProperties.disabled = isBusy || !activePage;\n  const activeLayer =\n    activePage?.layers.find((layer) => layer.layerId === currentNavigation?.activeLayerId) ?? null;\n  elements.applyLayerProperties.disabled = isBusy || !activeLayer;\n}\n""",
    """function updateStructureDisabledState() {\n  const pages = currentNavigation?.pages ?? [];\n  const activePage = pages.find((page) => page.pageId === currentNavigation?.activePageId) ?? null;\n  const activeLayer =\n    activePage?.layers.find((layer) => layer.layerId === currentNavigation?.activeLayerId) ?? null;\n  elements.deletePage.disabled = isBusy || pages.length <= 1 || !activePage;\n  elements.addLayer.disabled = isBusy || !activePage;\n  elements.deleteLayer.disabled =\n    isBusy || !activePage || activePage.layers.length <= 1 || !activeLayer || activeLayer.locked;\n  elements.deleteLayer.title = activeLayer?.locked\n    ? 'Unlock the layer before deleting it'\n    : 'Delete the active layer';\n  elements.applyPageProperties.disabled = isBusy || !activePage;\n  elements.applyLayerProperties.disabled = isBusy || !activeLayer;\n}\n""",
)

print("Applied locked-layer semantics fix")
