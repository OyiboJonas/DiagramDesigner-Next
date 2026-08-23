# ADR-018 — Editor commands and transient interaction model

- Status: Accepted for Phase 1
- Date: 2026-08-20
- Scope: editor core, viewport/tools, desktop/frontend integration

## Context

DiagramDesigner Next needs responsive pointer interaction, deterministic undo/redo and one global mutation model. The legacy application mixed mouse modes, object mutation and undo serialization inside the main form. Repeating that coupling in a Tauri/frontend architecture would create two undesirable extremes:

1. mutate the persistent document on every pointer move and route the hot path through history/IPC; or
2. let individual tools/views mutate `next-domain` directly and try to reconstruct history afterward.

Both make undo, dirty state, autosave and later collaboration/repair logic fragile.

## Decision

### Persistent document mutation goes through commands

Normal editor code does not receive unrestricted mutable access to the `next-domain::Document` owned by an editor session.

Persistent edits are expressed as typed commands. The current Phase-1 foundation supports move, bounds, rotation, top-level element creation and semantic deletion. Later commands follow the same boundary for style, text, grouping/ungrouping, layer/page operations and connector edits.

Each command is applied only after complete preflight validation. Multi-element commands and command transactions must either succeed entirely or leave the document unchanged.

### Pointer hot paths are transient

Pointer movement is **not** a stream of persistent document commands.

A drag/resize/rotate gesture follows this model:

```text
pointer down
  → capture interaction start state
  → pointer moves update transient frontend preview only
  → renderer displays base document + transient transform
  → pointer up / commit
  → emit one persistent editor command
```

The frontend therefore does not require an IPC round-trip for every pointer event. The Rust/application/editor boundary receives the committed semantic change, not raw pointer motion.

Keyboard nudges or future text editing may use command transactions/coalescing when repeated semantic commits are desirable. `EditTransaction` establishes that mechanism in the command/history layer; tools still must not bypass it with direct document mutation.

### Transactions are semantic and atomic

`EditTransaction` represents one persistent history step containing one or more semantic `EditCommand` values. It does not contain raw pointer events.

Adjacent geometric commands for the same target are coalesced before mutation:

- repeated `MoveElements` commands for the same element set accumulate their delta and cancel if the net delta returns to zero;
- repeated `SetBounds` for the same element keeps the final bounds;
- repeated `SetRotation` for the same element keeps the final rotation.

Applying a transaction records one undo/redo history entry. If any command fails, all commands already applied by that transaction are rolled back. Structural transactions are validated through `next-domain` before they are accepted into history.

### Create/delete are structural commands, not scene shortcuts

`CreateElement` inserts one top-level element into a specific master or page layer and records its root z-order. Duplicate element/port IDs, invalid geometry, locked layers and invalid insertion indices are rejected before the structural mutation is accepted.

Non-empty group construction is deliberately **not** smuggled through `CreateElement`; grouping remains a separate future semantic operation because child ownership and root membership must be explicit.

`DeleteElements` is also semantic:

- deleting a group expands to its complete owned descendant closure;
- deleting a child while a surviving group still owns it is rejected;
- surviving connectors that referenced deleted targets are detached while preserving their free endpoint positions;
- undo restores removed element storage, root z-order and detached connector relationships.

This prevents hidden orphan state and keeps structural edits reversible without serializing DDNX snapshots.

### Selection is transient editor state

Selection and hover/focus state are not document data and are not undoable mutations. Selection uses stable document-global `ElementId` values and is validated against the current document.

Structural commands, undo and redo prune selection entries that no longer exist instead of persisting dangling editor state.

### Undo/redo does not serialize DDNX

History stores typed forward transactions and inverse editor steps, not full DDNX/JSON/layer snapshots. DDNX remains the persistence boundary, not the undo implementation.

This avoids coupling history performance and semantics to file-format serialization.

### Dirty state is a history-state relationship

Dirty state is not a boolean toggled by tools and is not based on command count.

The history has stable state identifiers:

```text
initial state = 0
edit          = state 1
save          = saved state 1
edit          = state 2
undo          = state 1 → clean again
redo          = state 2 → dirty again
```

Autosave/recovery metadata remains separate from this user-visible saved-state relationship.

### Geometry has explicit units

Viewport and hit-test code must not use ambiguous `Point` values for both pixels and millimetres.

- document geometry remains in millimetres through `next-domain`;
- screen-space types explicitly use `*_px` fields;
- one `ViewportTransform` owns screen↔document conversion;
- zoom about a screen anchor preserves the document point under that anchor;
- pan modifies viewport origin only and never document coordinates.

### Master layers use the same edit path

Native `Document.master_layers` are not a special legacy mode. Their elements use the same command validation and mutation path as page-local elements. Locking rules apply equally.

## Consequences

### Positive

- pointer interaction stays responsive and frontend-local;
- undo/redo is semantic and independent of DDNX serialization;
- all persistent edits have one validation/history boundary;
- failed multi-command edits cannot leave partial mutations;
- structural create/delete remains reversible with stable z-order and connector repair;
- dirty state naturally becomes clean again when undo returns to the saved state;
- renderer implementations can consume base document plus transient interaction state without owning document mutation;
- future tools extend global commands instead of inventing local mutation paths.

### Cost

- every new persistent editor operation needs a command definition and inverse/transaction semantics;
- transient preview state must be modeled explicitly in the frontend/renderer layer;
- dedicated semantic commands are still required for grouping/ungrouping and the remaining document operations.

## Phase-1 implementation checkpoint

`editor-core` establishes:

- `EditorSession` as the renderer/platform-independent document session;
- validated active page/layer state and transient selection;
- move, bounds and rotation commands;
- `EditTransaction` as one atomic undo/redo history step;
- adjacent geometric-command coalescing;
- rollback of previously applied commands when a transaction fails;
- top-level `CreateElement` with explicit layer target and root z-order insertion;
- semantic `DeleteElements` with group-descendant expansion;
- rejection of direct child deletion while a surviving group owns it;
- connector detachment on target deletion and restoration on undo;
- selection pruning after structural edit/undo/redo;
- structural post-validation through `next-domain`;
- typed undo/redo;
- saved/current history-state tracking;
- the same command path for master and page-local layers.

`editor-geometry` establishes:

- explicit screen point/delta/size types;
- uniform viewport transform;
- document↔screen conversion;
- pan and anchor-preserving zoom;
- visible document rectangle calculation;
- normalized rectangle primitives;
- rotated-rectangle hit testing;
- point-to-segment distance for connector hit-test foundations.

## Follow-on requirements

- dedicated grouping / ungrouping commands;
- style/text/layer/page command families;
- connector-edit command families beyond delete-time detachment;
- frontend transient interaction state and pointer-capture lifecycle contract;
- renderer integration consuming base document plus transient overlays;
- target-Windows/WebView benchmark from ADR-019 before SVG is declared the primary renderer;
- autosave/recovery state kept separate from undo and user-visible saved state.

Tracks #11.
