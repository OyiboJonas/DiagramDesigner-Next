from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    if text.count(old) != 1:
        raise SystemExit(f"Expected one anchor in {path}, found {text.count(old)}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "apps/desktop/ui/app.js",
    """    const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n    elements.deleteSelection.disabled = selectionCount === 0;\n    elements.applyProperties.disabled = !currentSelectionProperties?.primary;\n    updateStructureDisabledState();\n""",
    """    const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n    const primary = currentSelectionProperties?.primary ?? null;\n    elements.deleteSelection.disabled = selectionCount === 0;\n    elements.applyProperties.disabled =\n      !primary || (primary.geometryEditable === false && primary.textEditable !== true);\n    updateStructureDisabledState();\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """  const activeTool = connectorTool;\n  svgSurface.setConnectorTool(null);\n  setBusy(true);\n""",
    """  setBusy(true);\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """  } finally {\n    setBusy(false);\n    if (connectorTool === activeTool && activeTool !== null) {\n      svgSurface.setConnectorTool(activeTool);\n    }\n  }\n}\n\nasync function syncRecovery() {\n""",
    """  } finally {\n    setBusy(false);\n  }\n}\n\nasync function syncRecovery() {\n""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """  let connectorController = null;\n  let connectorTool = null;\n  let selectedElementIds = [];\n""",
    """  let connectorController = null;\n  let connectorTool = null;\n  let connectorCommitPending = false;\n  let selectedElementIds = [];\n""",
)

replace_once(
    "apps/desktop/ui/candidate-svg-surface.mjs",
    """      controller: connectorController,\n      getConnectorKind: () => connectorTool,\n      onOverlay: (preview) => renderConnectorPreview(svg, preview),\n      onCommit: (commit) => {\n        renderConnectorPreview(svg, { ...commit, kind: \"connector-preview\" });\n        Promise.resolve(commitConnector(commit)).catch((error) => {\n          removeConnectorPreview(svg);\n          onError(error);\n        });\n      },\n""",
    """      controller: connectorController,\n      getConnectorKind: () => (connectorCommitPending ? null : connectorTool),\n      onOverlay: (preview) => renderConnectorPreview(svg, preview),\n      onCommit: (commit) => {\n        renderConnectorPreview(svg, { ...commit, kind: \"connector-preview\" });\n        connectorCommitPending = true;\n        Promise.resolve()\n          .then(() => commitConnector(commit))\n          .catch((error) => {\n            removeConnectorPreview(svg);\n            onError(error);\n          })\n          .finally(() => {\n            connectorCommitPending = false;\n          });\n      },\n""",
)

print("Applied connector UI state fixes")
