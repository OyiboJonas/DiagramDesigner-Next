# DiagramDesigner Next 0.1.0-alpha.1 — Windows alpha guide

This is the first functional alpha of DiagramDesigner Next. It is an unsigned portable Windows build for evaluation and compatibility testing, not a production installer.

## What this alpha is meant to prove

The alpha should support a complete small-diagram workflow without falling back to classic Diagram Designer:

- create and edit Rectangle, Ellipse and simple Text elements;
- draw straight and orthogonal connectors and attach/edit their endpoints on shape ports;
- select one or many elements, move them with snapping, resize and rotate normal geometry;
- change basic shape stroke/fill and simple text colour;
- manage pages and page-local layers;
- Undo/Redo persistent edits;
- save native `.ddnx`, reopen it, and recover unsaved state after an interrupted/dirty session;
- import legacy `.ddd` and `.ddt` as detached, unsaved Next copies that can only be saved as `.ddnx`.

## Capabilities added after the original alpha baseline

The `0.1.0-alpha.1` version label is still used while the native alpha smoke gate remains open, but current `main` builds include additional editor-productivity and persistence work that was intentionally outside the original baseline:

- structured Copy/Paste/Duplicate with deterministic offset, fresh identities and connector remapping;
- z-order controls for top-level elements and structural groups;
- Group/Ungroup with logical group selection and atomic group movement;
- group-aware Copy/Paste/Duplicate that preserves nested group hierarchy and internal references;
- explicit Save As with `Ctrl/Cmd+Shift+S`, plus native confirmation before a newly selected existing `.ddnx` destination is replaced.

These additions are covered by repository, frontend, DDNX and Windows/Tauri automation. They do **not** replace the outstanding human Windows GUI smoke required by the alpha-readiness gate, so the native smoke path below remains focused on the original alpha baseline.

## Native Windows alpha smoke path

1. Extract the workflow artifact to a normal local folder and start `DiagramDesigner-Next-Alpha.exe`.
2. Windows may display SmartScreen because the executable is not code-signed. Continue only for an artifact obtained from this repository's `Windows desktop preview` workflow.
3. Create a Rectangle, Ellipse and Text element.
4. Change a shape's fill colour, stroke colour and stroke width; change the Text element's colour.
5. Resize and rotate one shape. Hold Shift during rotation and verify 15-degree snapping.
6. Multi-select at least two elements with Ctrl/Shift and move them together. Verify Grid/Snap behavior.
7. Draw a straight connector between two shape ports and move one connected shape. Verify the connected endpoint follows the port.
8. Draw an orthogonal connector and edit one endpoint back to a free endpoint and onto another port.
9. Exercise Ctrl+Z / Ctrl+Shift+Z (or Ctrl+Y), then Ctrl+S.
10. Save to a new `.ddnx`, close the app, reopen the file and verify geometry, appearance and connections.
11. Import a legacy `.ddd` or `.ddt`. Verify the UI identifies it as an imported copy and that Save asks for a new `.ddnx` destination rather than modifying the legacy source.
12. Make an unsaved edit and close the app. Restart it and verify the recovery decision appears; restore the snapshot and verify the recovered copy is still unsaved until explicitly saved.

For current builds, also verify that Save As opens a new DDNX destination picker and that choosing an already-existing DDNX asks for explicit native confirmation. Choosing No/Cancel must leave the document and its current save association unchanged. A normal Save on an already-associated native DDNX continues to replace that exact file atomically without an additional overwrite prompt.

## Alpha close/recovery behavior

Closing the main window with dirty, imported or recovered state does **not** silently discard the current document. Before the native window is allowed to close, the Rust desktop boundary writes a fresh, atomically persisted DDNX recovery checkpoint. On the next start, the existing recovery flow offers Restore or Discard. This checkpoint is not a normal Save and does not overwrite the user's document or legacy source.

If that recovery checkpoint cannot be written, the close request is blocked and the status bar reports the recovery error.

## Known alpha limitations

- Windows only for this alpha; no installer and no code signing.
- Legacy `.ddd`/`.ddt` support is import-only. DiagramDesigner Next never writes those formats.
- Rich/mixed legacy text can be displayed but the basic text field deliberately refuses destructive flattening of unsupported formatting or dynamic fields.
- Basic appearance currently covers shape stroke/fill and text colour. Gradients, dash styles, connector markers and advanced text formatting are not editor controls yet.
- The executable depends on the Windows WebView2 runtime, normally present on current Windows 10/11 systems.

## Failure information

For a reproducible problem record:

- the operation immediately before the failure;
- source file type (`.ddnx`, `.ddd`, `.ddt`, or new document);
- visible error/status text;
- whether Undo/Redo or restart/recovery changes the result;
- `version`, `source_commit` and `desktop_cargo_lock_blob` from `BUILD.txt`.

Do not add private/company legacy files to this public repository unless they are explicitly cleared for publication.
