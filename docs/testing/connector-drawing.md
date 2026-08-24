# Connector drawing preview checks

This preview slice adds free-endpoint straight and orthogonal connector drawing to the desktop editor.

## Manual smoke

1. Start the Windows preview and open or create a document with a visible, unlocked page-local layer.
2. Select **Line**, drag across the page, and release. A straight connector should remain selected after creation.
3. Select **Orthogonal**, drag across the page, and release. The transient preview and committed route should follow the same free-endpoint axis convention.
4. Draw several connectors without reselecting the tool. The active connector tool remains armed until it is toggled off or **Escape** is pressed.
5. Press **Escape** during a drag. No connector should be added and document history must not advance.
6. Lock or hide the active layer. Connector tools must become unavailable.
7. Select a connector and move or delete it. Undo/redo must restore the semantic create/move/delete operations.
8. The basic inspector may show connector bounds for reference but must not offer generic resize/rotation editing for connector geometry.
9. Save as `.ddnx`, reopen it, and verify the connectors remain present.

## Scope boundary

Endpoints are free points in this slice. Attaching endpoints to element ports/anchors and editing endpoint connections are separate semantic operations and are intentionally not stored as frontend-only state.
