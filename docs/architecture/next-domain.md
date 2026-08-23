# DiagramDesigner Next domain boundary

Status: Phase 0 architectural contract for issue #7.

## Purpose

`next-domain` is the persistent, renderer-independent model of DiagramDesigner Next. It is deliberately separate from:

1. the legacy byte codec (`legacy-ddd`),
2. the legacy-to-Next converter (`legacy-migrate`),
3. editor/application state,
4. renderers,
5. the DDNX package codec.

The dependency direction is one-way:

```text
DDD / DDT bytes
    ↓
legacy-ddd
    ↓
validated legacy intermediate model
    ↓
text normalization + reference resolution
    ↓
legacy-migrate
    ↓
next-domain
    ↓
DDNX / editor / renderer
```

No editor, renderer or DDNX implementation may depend on Delphi/VCL types, legacy object-list indices or the legacy binary layout.

## Stable identity

Every independently referenceable domain entity has a typed UUID:

- `DocumentId`
- `TemplateId`
- `PageId`
- `LayerId`
- `ElementId`
- `PortId`
- `StyleId`
- `AssetId`

Legacy imports derive deterministic UUIDv5 values from the source file SHA-256 plus the importer path. Repeated conversion of the identical source therefore produces deterministic identities without pretending that DDD/DDT contained UUIDs.

Once a Next document is saved, those IDs are document identity. They must not be regenerated from array positions or storage location.

### Document-global identity namespace

For a `Document`, `PageId`, `LayerId`, `ElementId`, `PortId`, `StyleId` and `AssetId` are document-wide identities. In particular, `ElementId` and `PortId` are not scoped to a `Scene`.

This allows an element to move between page-local layers or between page and master scope without changing identity. `NextArtifact::validate()` carries shared element/port ID sets through all document scenes and rejects collisions across layer boundaries.

Relationship scope is a separate rule: global identity does not implicitly permit arbitrary cross-layer group or connector references. Current group/connector relationship validation remains scene-local until a dedicated cross-layer semantics decision is made. See ADR-012.

## Geometry and units

The Next model stores document geometry in millimetres.

The legacy conversion boundary uses the source constant:

```text
DesignerDPmm = 2520 legacy units / mm
```

Conversion occurs once in `legacy-migrate`; legacy integer coordinates do not leak into normal editor geometry.

Normalized ports and polygon/link coordinates remain normalized floating-point coordinates where that is their semantic representation.

## Document structure

A document contains:

```text
Document
 ├─ defaults
 ├─ master_layers[]
 │   └─ Scene
 │       ├─ roots[]
 │       └─ elements[]
 ├─ pages[]
 │   └─ layers[]
 │       └─ Scene
 │           ├─ roots[]
 │           └─ elements[]
 ├─ styles[]
 ├─ assets[]
 └─ import metadata
```

`master_layers` are normal document layers rendered before page-local layers on every page. They use the same `Layer`, `Scene`, element, style, asset and identity rules as page-local layers.

The original DiagramDesigner DDD container `Stencil` is source-verified as exactly this kind of shared background content: the legacy renderer draws it before the current page's local layers. Consequently `legacy-migrate` maps a non-empty DDD `Stencil` to `Document.master_layers`; the normal Next domain and DDNX package contain no legacy-specific Stencil type. See ADR-011.

A `Scene` keeps top-level z-order in `roots`, while all elements are stored flat by stable `ElementId`. Groups reference child IDs rather than owning nested pointer lists. Moving an element into or out of a group must not require changing identity.

## Elements

The current element union covers all 12 source-defined legacy object families without using their numeric IDs as the new type system:

- text,
- rectangle / rounded rectangle,
- ellipse,
- straight connector,
- orthogonal connector,
- raster image,
- Windows metafile asset,
- group,
- polygon,
- flowchart shape,
- curve,
- inherited-layer reference.

The original numeric type ID remains only in `ElementImportMetadata` for traceability.

## Ports and connectors

Ports have stable `PortId` values and a logical index. Connector endpoints reference:

```text
Connection {
  element_id,
  port_id
}
```

They never reference a legacy owner-list position.

For imported standard shape ports, `legacy-migrate` follows the original DiagramDesigner order:

1. centre `(0.5, 0.5)`,
2. left `(0.0, 0.5)`,
3. right `(1.0, 0.5)`,
4. top `(0.5, 0.0)`,
5. bottom `(0.5, 1.0)`.

Custom rectangle links, polygon vertices and serialized picture/group links become explicit ports from their stored normalized coordinates.

Connector start/end free positions are retained even when connected, providing a deterministic fallback when an endpoint is detached or a connection later becomes invalid.

Legacy marker and line-style constants are mapped to typed enums. Unknown historical/future values remain `Custom(code)` instead of being silently coerced.

Curve kinds are typed as Catmull-Rom, legacy, Bezier or line segments. Route geometry and endpoint relationships remain separate concepts.

The legacy connector field `FFillColor` is represented as connector `secondary_color`. Source inspection shows that the same value serves outline-line interiors, marker/UML fill cases and solid connector-label background. Renderers therefore consume one typed semantic rather than platform-specific lookups.

## Text

Rich text is Unicode and token based. It contains no Delphi backslash markup.

```text
raw Delphi bytes
  → explicit charset decision
  → Unicode
  → legacy markup parser
  → Next RichTextDocument
```

Action and hint tails are retained as typed inert values. They are not executable actions until a later application/security layer explicitly classifies them.

Symbol-font-dependent glyphs remain explicit `SymbolGlyph` tokens where a portable Unicode meaning is not yet proven.

`TextBlock.layout` stores typed horizontal alignment, typed vertical alignment and metric margin. Unknown historical alignment values remain `LegacyUnknown(value)` rather than disappearing into renderer-local conditions.

## Styles, colours and gradients

Stroke, fill and text colour are domain values.

Ordinary Windows `TColor` RGB values become portable RGBA. System palette references remain explicit `SystemPalette(index)` values rather than asking a cross-platform renderer to interpret a Windows integer.

Legacy gradients become `LinearGradient` with portable end colour and typed `GradientAxis` (`AlongX` / `AlongY`). Legacy packed bits are consumed once at the migration boundary.

Raw values needed for migration auditing remain in import metadata, not normal editor state.

## Assets

Raster and metafile payloads become content-addressed document assets with `AssetId` and SHA-256 content identity.

The migration layer retains raster/metafile payloads losslessly. Platform-specific metafile conversion to a portable rendering representation is a later import/platform stage; it is not performed implicitly by the domain model.

## DDT palettes

A `.ddt` file becomes `TemplatePalette`, not a fake one-page document. It uses the same scene, element, style, asset and stable-ID model as documents.

This keeps template/library semantics distinct while allowing future editor tooling to instantiate templates without a second object model.

## Validation

`NextArtifact::validate()` checks structural invariants including:

- schema version,
- document-wide duplicate page/layer/element/port/style/asset IDs,
- root element existence,
- group child existence,
- style references,
- asset references,
- connector target element existence,
- connector target port existence.

For `TemplatePalette`, the same element/port uniqueness rules apply to its single scene.

`legacy-migrate` refuses to return an artifact that fails these checks. For DDD input it also refuses an invalid second-pass legacy object/link reference graph.

## Private/external real-file regression

`dd-migrate verify-corpus` is the reproducible regression boundary for private files that must not be committed to GitHub. It verifies a source SHA-256 supplied by an external manifest, performs bounded inspection, converts through the normal `legacy-migrate` path, validates the resulting Next artifact and computes a deterministic SHA-256 over compact Next-domain JSON.

The committed `fixtures/private-corpus.example.json` is deliberately generic. Real private manifests, fixture names, paths, source fingerprints, decoded contents and document-specific structural fingerprints remain outside the public repository. Usage is documented in `docs/phase-0/private-corpus-verification.md`.

## Import metadata boundary

Import metadata may contain source format/version/hash, source object path/type ID, original anchor bits, selected raw legacy values and migration diagnostics.

This data exists for auditability, compatibility work and repair tooling. It must not become the primary editor state.

## Current deliberate limitations

The remaining Phase-0 limitations are explicit:

- a redistributable real non-empty DDD Stencil fixture is still desirable even though source semantics, synthetic conversion and DDNX master-layer round-trip coverage are implemented;
- `InheritedLayer` still retains its relative page/layer indices until stable cross-page/layer resolution is validated with a redistributable real type-12 fixture;
- Symbol-font glyphs are not guessed into Unicode;
- Windows metafile bytes are retained losslessly but are not yet converted to a portable vector representation;
- private real-world corpus replay remains an external/local evidence process and its identifying metadata is intentionally not published.

None of these cases is silently discarded.

## Rule for future code

New editor or renderer code consumes `next-domain` only. If it needs a concept currently available only in `ElementImportMetadata`, decide whether that concept belongs in the typed domain and promote it globally. Do not add a local DDD/DDT special case to UI, renderer or page code.
