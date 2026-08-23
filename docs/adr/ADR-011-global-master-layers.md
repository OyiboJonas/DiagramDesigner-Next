# ADR-011 — Global master layers

- Status: Accepted for pre-v1 domain
- Date: 2026-08-19
- Scope: `next-domain`, legacy migration, DDNX, future renderer/editor

## Context

The original DiagramDesigner DDD container serializes one `Stencil` layer at container level. Source inspection of the pinned reference implementation shows that this layer is drawn before the active page's local layers on every page.

That behavior is materially different from a reusable template palette (`.ddt`). Treating the DDD `Stencil` as a template/library would therefore misrepresent the legacy semantics and would force page/rendering exceptions later.

At the same time, the new architecture must not encode a Delphi field name or a one-off importer concept into normal editor state.

## Decision

DiagramDesigner Next has a native document-level collection:

```text
Document.master_layers: Vec<Layer>
```

Master layers:

- use the same `Layer`, `Scene`, `Element`, style, asset and connection model as page-local layers;
- are rendered before page-local layers on every page;
- share the same `LayerId` namespace as page-local layers;
- preserve collection order as their z-order relative to one another;
- are normal editable Next document state, not import metadata;
- may contain more than one layer even though the current legacy DDD source has a single container `Stencil`.

The legacy converter maps a non-empty DDD `Stencil` to a Next master layer. It retains the source path `stencil/...` only for deterministic imported UUIDs, text-normalization lookup and audit metadata.

DDNX persists `master_layers` directly in the document projection. DDNX contains no special `Stencil` field.

## Rendering contract

For each page, the renderer's logical draw order is:

```text
Document.master_layers[0..n]
Page.layers[0..n]
```

Visibility and later editor locking behavior are layer properties. The renderer must not ask whether a master layer originated from a legacy file.

## Validation contract

Master and page-local layers share one document-wide `LayerId` namespace. `NextArtifact::validate()` therefore rejects duplicate layer IDs across both collections.

Each master-layer scene is validated with the same root/group/style/asset/connector-reference invariants as a page-local scene.

## Consequences

### Positive

- faithful mapping of the legacy global-background behavior;
- no renderer-side DDD special case;
- supports native Next master/background/watermark/title-frame use cases beyond legacy compatibility;
- avoids incorrectly conflating `.ddd` Stencil and `.ddt` template-palette semantics;
- DDNX remains a serialization of the Next domain rather than a mirror of Delphi fields.

### Costs

- page rendering must compose document-global and page-local layer stacks;
- editor UX must later make the distinction between master and page-local layers explicit;
- operations that move a layer between scopes need deliberate semantics while preserving stable IDs;
- inherited-layer reference resolution must account for page-local versus master scope when that legacy type is promoted to stable references.

## Alternatives rejected

### Keep `Stencil` as an importer-only field

Rejected because renderer/editor code would then need a local legacy exception or the content would be lost after native save.

### Convert the Stencil into every page

Rejected because it duplicates elements/assets/styles, breaks identity, changes editing semantics and would diverge as pages are edited.

### Treat Stencil as a template palette

Rejected because the original source renders it automatically on every page; a template is an instantiation source, not persistent shared page content.

### Allow exactly one master layer

Rejected because that would unnecessarily preserve a legacy storage limitation in the new domain. A collection is the more general global component and does not complicate the current one-layer import.

## Verification

Required regression layers:

1. domain validation test for shared master/page `LayerId` namespace;
2. DDNX round-trip with a non-empty native master layer;
3. synthetic legacy DDD Stencil → Next master-layer conversion;
4. real non-empty legacy DDD Stencil fixture when available;
5. full existing DDT compatibility/persistence corpus after domain changes.
