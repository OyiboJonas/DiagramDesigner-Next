# ADR-012 — Document-global element and port identity

- Status: Accepted and implemented for pre-v1 domain
- Date: 2026-08-19
- Scope: `next-domain`, editor commands, persistence, import

## Context

Elements are stored inside per-layer `Scene` values, and groups/connectors currently resolve relationships within a scene. The initial validator therefore checked duplicate `ElementId` and `PortId` values while validating each individual scene.

That is insufficient for the meaning of the IDs in DiagramDesigner Next. An element may later move between page-local layers or between page and master scope without changing its identity. DDNX persists IDs independently of array positions. Document tools, selection state, history and future cross-layer operations must be able to treat an `ElementId` as unambiguous without also carrying a layer-qualified key.

## Decision

For a `Document`:

- `PageId` is unique across the document;
- `LayerId` is unique across all master and page-local layers;
- `ElementId` is unique across all scenes in all master and page-local layers;
- `PortId` is unique across all document elements;
- `StyleId` is unique across document styles;
- `AssetId` is unique across document assets.

For a `TemplatePalette`, element/port uniqueness applies across its single scene.

`NextArtifact::validate()` enforces the document-wide element and port namespaces by carrying shared ID sets through every master and page-local scene validation. Scene-local maps remain separate and are used only for relationship validation inside that scene.

Scene-local relationship rules remain separate. A globally unique ID does **not** by itself permit a connector or group to target arbitrary content in another layer. Cross-layer relationship semantics require an explicit future domain decision.

## Consequences

### Positive

- moving an element between layers does not require changing identity;
- selection/history/indexing can key by `ElementId` without a `(LayerId, ElementId)` composite;
- corrupt DDNX documents cannot hide duplicate identities in separate layers;
- imported deterministic UUIDv5 paths remain globally unambiguous;
- master layers do not create a second identity namespace.

### Costs

- document validation carries shared element/port ID sets across scene validation;
- copy/duplicate commands must always mint new IDs even when copying between layers;
- paste/import code must remap colliding IDs before commit.

## Rejected alternative

### Scene-scoped identity

Rejected because it would make identity dependent on storage location and complicate layer moves, undo/redo, selection state and future document-wide indexing.

## Verification

Implemented domain tests verify that:

1. duplicate `ElementId` values across master and page-local layers are rejected;
2. duplicate `PortId` values on elements in different layers are rejected;
3. duplicate `LayerId` values across master and page-local scope remain rejected;
4. normal single-scene/template validation continues through the same validator;
5. the full imported DDD/DDT and DDNX compatibility corpus remains the regression gate after this change.

The branch CI runs format, Clippy, workspace tests and the pinned 30-palette Legacy → Next → DDNX → Next corpus on every relevant change.
