# Frontend interaction contract

Status: Phase 1 working contract  
Date: 2026-08-27

## Purpose

DiagramDesigner Next keeps high-frequency pointer interaction inside the WebView/frontend. Persistent editor state remains owned by `editor-core` and is mutated only through semantic commands/transactions.

This document makes the boundary concrete for move gestures and provides the pattern for resize, rotation and connector manipulation.

## Move gesture lifecycle

1. The frontend performs hit testing and resolves the drag selection.
2. `pointerdown` starts a local move gesture and acquires browser pointer capture.
3. Every `pointermove` is converted from screen coordinates to document millimetres in the frontend and updates only a transient overlay.
4. Optional snapping/constraint logic may transform the transient document delta before display.
5. `pointerup` releases capture and emits at most one semantic `move-elements` intent containing stable element IDs and the final document-space delta.
6. The application adapter converts that intent into one `EditCommand::MoveElements` call (or equivalent single-command transaction) in `editor-core`.
7. `pointercancel`, `lostpointercapture`, component disposal, or an interaction error clear the overlay and do not mutate the document.

## Non-negotiable invariants

- No raw `pointermove` event crosses desktop IPC.
- No `pointermove` creates an Undo/Redo history entry.
- No transient overlay changes `HistoryStateId`, `PageStateKey`, dirty state, recovery state, or DDNX content.
- Persistent geometry is committed only once at successful gesture end.
- Zero-distance gestures produce no persistent command.
- The renderer does not own gesture state. SVG/Canvas/WebGL adapters consume persistent render plans plus separately supplied transient overlay information.
- Selection policy and hit testing remain outside the generic pointer binding so renderer/application-specific details do not leak into the lifecycle primitive.
- Pointer capture belongs to the DOM-facing binding, not to `editor-core` or `editor-runtime`.

## Current implementation

`web/editor-interaction/move-gesture.mjs` provides:

- `MoveGestureController` — framework-free transient gesture state;
- `bindMovePointerSurface` — browser Pointer Events + capture/release lifecycle;
- immutable `move-preview` overlay values;
- immutable `move-elements` semantic commit values;
- an explicit `transformDelta` extension point for future grid/object snapping and constraints.

`web/editor-interaction/move-gesture.test.mjs` covers:

- transient updates followed by one final commit;
- non-owning pointer rejection;
- cancellation and lost-capture behavior;
- zero-distance no-op behavior;
- snapping/constraint hook behavior;
- DOM pointer-capture/release lifecycle;
- invalid input rejection.

## Application adapter mapping

The future desktop/application adapter must map a completed frontend intent conceptually as follows:

```text
move-elements {
  elementIds: [stable IDs...],
  deltaMm: { x, y }
}
        |
        v
EditCommand::MoveElements {
  element_ids,
  delta_mm: Point { x, y }
}
```

The adapter must validate IDs/geometry and call the existing `EditorSession::execute` / transaction boundary once. It must not expose a generic arbitrary-command IPC endpoint to untrusted WebView code.

## Discrete arrange actions

Alignment and distribution are discrete semantic actions rather than pointer gestures. The frontend may enable or disable their controls from selection count, but it must not calculate final element positions or reproduce layout rules. It sends the selected stable element IDs plus the requested arrange operation through the application adapter; `editor-core` owns canonical document-space visual bounds, group expansion, connector synchronization and the single Undo/Redo history step.

For horizontal or vertical distribution, `editor-core` defines the edge-case semantics as follows:

- the leading anchor is the selected logical object with the smallest left/top visual edge, with the stable element ID as the deterministic tie-breaker;
- the opposite anchor is a distinct selected logical object chosen from the remaining objects by the farthest right/bottom visual edge; leading edge and stable element ID provide deterministic tie-breakers;
- intermediate logical objects retain leading-edge order and are placed at equal visual gaps between those anchors;
- a negative computed gap is valid and represents equal overlap, including containment cases;
- structural groups participate as one logical object using canonical subtree visual bounds and expand only when the resulting movement is committed.

The frontend must not duplicate these rules. This keeps restricted WebView intents caller-order-independent and prevents UI geometry from diverging from `EditCommand::ArrangeElements`.

## Follow-on gestures

Resize, rotation and connector dragging should reuse the same lifecycle shape:

- local pointer capture;
- transient frontend preview;
- optional snapping/constraints;
- cancellation with no persistent mutation;
- one typed semantic commit on successful completion.

Their persistent commit types should remain explicit (`SetBounds`, `SetRotation`, dedicated connector command, or future affine group command) rather than introducing a generic mutation payload.
