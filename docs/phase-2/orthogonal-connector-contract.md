# Orthogonal connector compatibility contract

Phase 2 promotes legacy axis-aligned connectors from a typed unsupported primitive to rendered SVG semantics without adding SVG-specific state to the domain, editor history, persistence, or `render-plan` item model.

## Public compatibility basis

The compatibility source is the pinned public Diagram Designer repository `meesoft/DiagramDesigner` at commit `12188325704b559c211addf82f26183098b0e201`, primarily `Application/LineObject.pas`:

- `TAxisLineObject.FindEndDirection` for endpoint direction selection;
- `TAxisLineObject.DrawShape` for route topology;
- `TAxisLineObject.DrawCornerArc` for solid/outline corner rounding;
- `TAxisLineObject.DrawCornerStyle` for styled-segment boundaries;
- `TAxisLineObject.Draw` for endpoint-marker direction and outer/inner outline ordering.

The implementation and tests use only this public source plus synthetic Next-domain fixtures. Private `.ddd` documents and private-corpus metadata are not part of the public contract.

## Existing domain information is sufficient

`ElementKind::OrthogonalConnector` already stores the two endpoints, marker styles, line style, secondary colour, and corner radius. An endpoint may also carry a stable `Connection` to a target element/port. Imported ports retain their normalized legacy link positions.

Consequently, endpoint direction and route geometry are derived at render time. No legacy-only direction field and no SVG path state is persisted in `next-domain` or DDNX.

## Endpoint direction

For an unconnected endpoint, and for an endpoint connected to another orthogonal connector, the route uses the same dominant-axis rule as upstream `FindEndDirection`:

- horizontal separation greater than vertical separation -> horizontal endpoint direction;
- otherwise -> vertical endpoint direction;
- the endpoint's position relative to the connector midpoint chooses the corresponding left/right or top/bottom direction.

For an endpoint connected to a normal element, its referenced normalized port selects direction as follows:

| Port position | Direction |
| --- | --- |
| `x < 0.05` | horizontal-left |
| `x > 0.95` | horizontal-right |
| `y < 0.05` | vertical-top |
| `y > 0.95` | vertical-bottom |
| interior port | dominant offset from target centre |
| exact centre | perpendicular to the other endpoint direction |

The two directions are resolved in the same order as upstream reset logic: start first, end second. This makes center-port ties deterministic without adding stored renderer state.

## Route families

The route follows the upstream `DrawShape` families:

| Endpoint directions | Route |
| --- | --- |
| one horizontal, one vertical | one orthogonal corner |
| both vertical, opposite directions | two-corner route split at midpoint Y |
| both horizontal, opposite directions | two-corner route split at midpoint X |
| both vertical, same direction | outer vertical dogleg |
| both horizontal, same direction | outer horizontal dogleg |

For same-direction endpoints the outward clearance is the maximum of:

1. the largest endpoint marker size group converted through the legacy point scale and multiplied by three;
2. the relevant corner diameter;
3. one tenth of the perpendicular endpoint separation.

Route points preserve upstream segment boundaries. In particular, collinear midpoint boundaries are not simplified away because legacy styled connectors call `DrawLineStyle` separately for each segment and therefore restart their dash pattern at those boundaries. Same-axis hairpin turnarounds are likewise retained.

## Corner radius

Corner rounding is active only for `LineStyle::Solid` and `LineStyle::Outline`, matching upstream behavior. Other line styles remain sharp and are rendered segment by segment.

SVG arcs are renderer-local materialization of the legacy radius rule. The radius is bounded by the available endpoint separation so a requested corner radius cannot create an unbounded route.

## Marker and outline reuse

Orthogonal connectors reuse the Phase-2 marker contract documented in `connector-marker-contract.md`. An invisible marker-carrier path follows the sharp orthogonal route, allowing SVG's path tangent to provide endpoint orientation while keeping marker geometry centralized.

`LineStyle::Outline` reuses the two-colour contract documented in `connector-outline-contract.md`:

1. full-width primary outer path;
2. coincident secondary-colour inner path at half width;
3. endpoint markers after both line passes.

This preserves the same paint-order invariant as straight connectors.

## Culling invariant

A same-direction dogleg can be visible outside the rectangle formed by its two serialized endpoints. Culling by `element.bounds_mm` alone is therefore unsafe.

`render-plan` now computes a conservative renderer-independent culling rectangle for orthogonal connectors and both paths use it:

- the cold `build_page_plan` viewport check;
- `PreparedPage` spatial-grid insertion and viewport query.

The conservative expansion intentionally may retain an off-screen connector earlier than strictly necessary. It must never drop a dogleg that can still intersect the viewport. This keeps cold and prepared rendering behavior equivalent without storing SVG routing state in `PreparedPage`.

## Explicit unsupported boundary

Known standard marker styles and known standard line styles are rendered. Unknown imported values remain explicit:

- `MarkerStyle::Custom(_)` -> `ConnectorMarkerDeferred`;
- `LineStyle::Custom(_)` -> `ConnectorLineStyleApproximated`.

No unsupported code is silently promoted to compatibility.

## Regression coverage

Public regression tests cover:

- same-side vertical hairpins and marker-clearance routing;
- connected-port direction resolution;
- preservation of styled segment-reset boundaries;
- rounded solid/outline geometry;
- secondary RGBA paint and half-width outline pass;
- reuse of standard endpoint markers and marker ordering;
- retention of typed diagnostics for custom marker/line codes;
- cold-plan and `PreparedPage` agreement when the viewport intersects only the conservative dogleg bounds.
