# ADR-020 — Structural groups and transform semantics

- Status: Accepted for Phase 1
- Date: 2026-08-20
- Scope: editor core, grouping/ungrouping, renderer interaction, future affine group transforms

## Context

DiagramDesigner Next already represents a group in `next-domain` as a structural element whose `children` are stable `ElementId` references. `render-plan` does not draw a group primitive; it expands the group and renders its children in declared order.

That representation creates an important architectural choice. A group can either become a hidden transform hierarchy that implicitly changes all descendant coordinates, or it can remain a structural ownership/order node while visible geometry stays in document coordinates.

Introducing implicit group-local transforms now would conflict with the current domain and renderer boundaries:

- existing imported and native element geometry is stored in document coordinates;
- connector endpoint positions and curve control points also contain absolute document-space geometry;
- `render-plan` expands groups structurally rather than composing group transforms;
- generic `SetBounds` / `SetRotation` operate on one element and therefore cannot correctly resize or rotate an entire descendant subtree;
- silently changing z-order while grouping would make an apparently structural edit alter document appearance.

Phase 1 therefore needs explicit grouping semantics before desktop tools are built on top of the command layer.

## Decision

### Groups are structural owners, not hidden transform nodes

A group owns an ordered list of direct children. Grouping itself does not change child coordinates, rotations, connector endpoints or curve control points.

The group element stores a derived document-space union bound for structural/editor use, starts with zero rotation, carries no style/text, and introduces no implicit local coordinate system.

Visible descendants remain authoritative in document coordinates.

### Group creation is a dedicated semantic command

Non-empty groups cannot be created through the generic top-level `CreateElement` command. They are created through `GroupElements` so ownership and sibling replacement happen atomically.

`GroupElements` requires:

- at least two elements;
- a fresh `group_id`;
- every selected element to be editable;
- all members to belong to the same layer and the same **direct sibling owner**;
- the members to form one contiguous range in that owner's sibling list.

A direct sibling owner is either:

- the scene root list; or
- one parent group.

Nested grouping is therefore supported, but only among direct siblings of the same parent.

### Grouping must not change z-order

The selected contiguous sibling range is replaced by exactly one group at the position of the first selected sibling. The group's `children` preserve the original sibling order.

Non-contiguous selections are rejected rather than compacted because compacting them would move intervening elements and silently alter draw order.

Undo restores the complete previous sibling list and removes only the created group. Redo reapplies the same semantic grouping command.

### Ungroup promotes children into the exact group position

`Ungroup` removes one structural group and replaces its single sibling slot with the group's ordered children.

The children keep their existing document-space geometry and ordering. The group element is removed from scene storage.

Undo reinserts the exact group element at its original storage position and restores the complete previous sibling list.

### Connections to a removed group are detached explicitly

A group may have ports in imported or future data. Removing the group through `Ungroup` must therefore not leave dangling `Connection` references.

Before the group element is removed, surviving connectors that target the group are detached while their free endpoint positions remain unchanged. Undo restores those exact connections.

Ungroup does **not** guess a replacement child/port target. Re-targeting would be a separate explicit editor operation.

### Moving a group translates its complete owned subtree

`MoveElements` remains the one semantic move command. When a selected target is a group, editor-core expands that target recursively to the complete descendant subtree before mutation.

Each concrete element is translated at most once, even if a command contains overlapping group/descendant targets.

The shared translation primitive moves all absolute geometry that belongs to an element:

- common `bounds_mm`;
- straight/orthogonal connector endpoint positions;
- curve control points;
- optional curve connector endpoint positions.

The structural group bounds are translated together with the descendants. Undo uses the same expanded target set and the same translation primitive with the inverse delta.

This preserves visible geometry and prevents group/container drift.

### Generic group resize and rotation are rejected

`SetBounds` and `SetRotation` reject structural groups with `GroupTransformRequiresDedicatedCommand`.

Changing only the group's stored rectangle or rotation would not transform its children and would therefore create invisible/stale container state. Phase 1 explicitly prefers rejection over misleading partial behavior.

A future group resize/rotation implementation must be a dedicated **affine group transform command** that defines and applies one coherent transform to the complete subtree, including connector endpoints and curve control points, with deterministic undo/redo semantics.

The future command must not be implemented as UI-specific child mutation.

### Ownership ambiguity is an error

Editor operations expect each grouped element to have one unambiguous structural owner. Ambiguous membership, malformed hierarchies or cycles are rejected/diagnosed rather than repaired implicitly inside a normal edit command.

Structural commands continue to run the complete `next-domain` validation before entering history.

## Consequences

### Positive

- grouping and ungrouping are structural, deterministic and reversible;
- grouping cannot silently change z-order;
- nested groups work through the same direct-owner model as root groups;
- children remain compatible with the existing renderer-independent document-coordinate geometry model;
- moving groups uses the same global geometry translation path as moving individual elements;
- connectors and curves cannot drift away from their visual/group position;
- undo/redo restores exact ownership/order rather than reconstructing it heuristically;
- dangling connector references are prevented when a group is removed;
- future affine transforms have a clear semantic boundary instead of accumulating special cases in tools or renderers.

### Cost

- arbitrary non-contiguous selections cannot be grouped without an explicit preceding z-order operation;
- group resize and rotation are intentionally unavailable until the dedicated affine transform command exists;
- group bounds are derived structural data and must stay synchronized whenever descendants move;
- editor code needs direct-owner and subtree traversal helpers rather than treating all scene elements as flat roots.

## Phase-1 implementation checkpoint

At checkpoint `119492028bbec01bafbe411a803ffae5956460ce`, `editor-core` implements:

- `GroupElements` and `Ungroup` semantic commands;
- root and nested direct-sibling ownership;
- contiguous-sibling validation to preserve z-order;
- derived union bounds for newly created groups;
- exact sibling-order restoration through undo/redo;
- recursive move-target expansion with duplicate suppression;
- shared translation of group descendants, connector endpoints and curve control points;
- explicit rejection of generic group resize/rotation;
- detachment of connections targeting an ungrouped group and exact restoration on undo;
- structural post-validation through `next-domain`.

The confirmed Rust 1.85 gate at this checkpoint is green with 25/25 `editor-core` tests, including dedicated coverage for grouping order/history, nested direct siblings, non-contiguous rejection, subtree movement, ungroup connection handling and rejected generic group transforms. The full workspace gate, renderer benchmark, SVG syntax check and pinned 30/30 legacy DDT corpus also pass.

## Follow-on requirements

- define a dedicated affine group transform command before enabling group resize/rotation;
- decide how affine transforms update ports and any future routed connector geometry;
- keep group transform previews transient in the frontend and commit one semantic command on gesture completion;
- maintain the renderer-independent structural expansion contract;
- keep snapping/alignment tools on document geometry rather than inventing a second group-local coordinate model.

Tracks #11 and implementation PR #12.
