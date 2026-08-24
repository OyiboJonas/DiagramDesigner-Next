# Phase 2 — connector marker rendering contract

## Scope

This contract is the first renderer-parity delivery after ADR-019 selected SVG for Phase 1. It covers endpoint markers on **straight connectors** only. Orthogonal connectors and curve connectors reuse the same marker semantics later, after their own geometry is implemented.

Private/company `.ddd` examples are not part of this public contract and must remain local. The compatibility reference here is the pinned public upstream Diagram Designer source.

## Public legacy reference

Pinned upstream source:

- repository: `meesoft/DiagramDesigner`;
- commit: `12188325704b559c211addf82f26183098b0e201`;
- files: `Application/LineObject.pas` and `Application/DiagramBase.pas`.

`TBaseConnectorObject.DrawLineEnd` defines the endpoint shapes. `DiagramBase.pas` defines `DesignerDPI = 64008`, `DesignerDPmm = DesignerDPI*10 div 254` and `DesignerDPpoint = DesignerDPI div 72`. DiagramDesigner Next stores geometry in millimetres, so the normalized marker vector uses one typographic point (`25.4 / 72 mm`) multiplied by the legacy marker size group.

## Standard marker mapping

| Domain marker | Legacy code family | Vector length | SVG geometry |
| --- | ---: | ---: | --- |
| `Stop` | `0x1*` | `1 pt` | perpendicular stop bar |
| `Circle` | `0x2*` | `2 pt` family | unfilled circle; radius `2 × line width` |
| `Ball` | `0x2*` | `2 pt` family | filled circle; radius `2 × line width` |
| `Diamond` | `0x2*` | `2 pt` | four-point diamond |
| `Arrow1` | `0x3*` | `3 pt` | open chevron |
| `Arrow2` | `0x3*` | `3 pt` | filled triangle |
| `Arrow3` | `0x3*` | `3 pt` | filled concave arrow |
| `DoubleArrow` | `0x4*` | `4 pt` | two filled triangles |
| `UmlIsA` | `0x5*` | `5 pt` | triangle with secondary-colour interior |
| `UmlHasA` | `0x5*` | `5 pt` | diamond with secondary-colour interior |
| `Many` | `0x6*` | `6 pt` | crow's-foot marker |

The marker tip is the connector endpoint. Geometry extends back into the connector. SVG uses `orient="auto-start-reverse"`, so the same normalized geometry points correctly at both the start and the end of a straight connector.

## Paint semantics

- normal open markers use the connector stroke colour and no fill;
- normal filled markers use the connector stroke colour for both outline and interior;
- UML `IsA` / `HasA` interiors use `Connector.secondary_color`, defaulting to white when absent;
- for legacy `Outline` line style, filled marker interiors use `Connector.secondary_color` and the marker outline is reduced relative to the main line width;
- `Arrow2`, `Arrow3` and `DoubleArrow` receive the legacy outline-style outward offset relative to the endpoint;
- system-palette colours retain the existing SVG fallback and typed diagnostic behaviour.

The straight-connector `Outline` line itself is now covered by the separate `connector-outline-contract.md`. For outline connectors, endpoint marker attributes are attached to an invisible carrier emitted after the outer and inner line passes so marker z-order matches the public upstream caller. This does not change the marker geometry or paint rules documented here.

## Diagnostic policy

All named standard `MarkerStyle` variants above are rendered and therefore no longer emit `ConnectorMarkerDeferred` through the public `render-svg` facade.

`MarkerStyle::Custom(_)` remains explicitly deferred. No arbitrary custom marker code is guessed or mapped to a standard shape. This preserves the project's rule that unsupported legacy semantics remain visible and typed rather than silently approximated.

## Renderer boundary

Marker support is implemented in the SVG renderer facade only. It does not add SVG state to `next-domain`, `render-plan`, editor commands or history. The Phase-1 evidence-tested core remains available behind the facade while Phase-2 parity features are added incrementally and regression-tested.
