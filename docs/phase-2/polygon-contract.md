# Phase 2 polygon rendering contract

## Scope

DiagramDesigner Next renders `ElementKind::Polygon` through the production SVG facade while keeping polygon geometry renderer-independent in `next-domain` and `render-plan`.

This contract is normalized from the pinned public Diagram Designer reference commit `12188325704b559c211addf82f26183098b0e201`, primarily:

- `Application/GroupObject.pas` — `TPolygonObject.Draw`, `SaveToStream`, `LoadFromStream`, `RescaleLinks`;
- `Application/ShapeObject.pas` — `TShapeObject.DrawPolygon`, `SetupGradientBrush`.

No private `.ddd` fixture or private-corpus metadata is part of this contract.

## Geometry

Legacy `TPolygonObject.Draw` treats polygon links as normalized coordinates relative to the object rectangle. Each point is materialized as:

- `x = bounds.left + normalized_x * bounds.width`
- `y = bounds.top + normalized_y * bounds.height`

DiagramDesigner Next preserves that model directly with `Vec<NormalizedPoint>` in `ElementKind::Polygon`.

The legacy load path stores the point count as a 16-bit value and then reads the normalized points. The public upstream resource string states that a polygon must have at least two points. Accordingly, the SVG renderer accepts two or more finite points. A one-point polygon is not silently approximated: it remains skipped and produces `SvgDiagnostic::InvalidGeometry`.

Vertices are not clamped to `[0, 1]` in the renderer. The legacy `RescaleLinks` operation normally normalizes edited polygons, but renderer input remains data-driven and finite out-of-range values are rendered relative to the element bounds rather than rewritten.

## Stroke and fill

Legacy polygons use the shared `TShapeObject.DrawPolygon` paint path. DiagramDesigner Next therefore reuses the same domain style semantics already used by other production SVG primitives:

- absent explicit style: black default stroke at `0.25 mm`, no fill;
- explicit `stroke: None`: no polygon outline;
- explicit stroke: document color plus the stored width;
- explicit `fill: None`: transparent interior;
- explicit fill: stored primary fill color;
- RGBA alpha is preserved as SVG opacity.

System-palette colors are not interpreted as platform-specific RGB values in the renderer. They use the established deterministic fallback paint and retain `SvgDiagnostic::SystemPaletteFallback` for traceability.

## Gradients

The public upstream shape renderer builds polygon gradients over the polygon bounds and uses the legacy gradient flag to select the gradient axis. The migration layer already normalizes that flag into `GradientAxis::AlongX` or `GradientAxis::AlongY`.

The SVG renderer therefore emits an object-bounded linear gradient:

- `AlongX`: start at the left edge and end at the right edge;
- `AlongY`: start at the top edge and end at the bottom edge.

Primary and end colors preserve RGBA alpha independently.

## Rotation

The polygon renderer uses the existing element rotation transform contract around the element-bounds center. Public legacy imports normally encode polygon rotation by rotating/rescaling the stored polygon points themselves, so imported polygons do not require a special legacy-only rotation state. Native Next documents can still use the renderer-independent `rotation_deg` field consistently with other elements.

## Z-order and culling

The Phase-1 SVG core previously skipped polygons with `UnsupportedPrimitive`. Phase 2 materializes valid polygon SVG in render-plan order without adding polygon-specific state to `render-plan`.

Polygon culling continues to use the existing renderer-independent element bounds and rotation-aware planning rules. No SVG-only culling cache is introduced.

## Diagnostics and support boundary

For a valid supported polygon:

- the SVG primitive is emitted;
- `rendered_elements` increases;
- `skipped_elements` decreases;
- the matching `UnsupportedPrimitive` diagnostic is retired.

For malformed polygon geometry:

- no SVG polygon is emitted;
- the element remains counted as skipped;
- `InvalidGeometry` is retained;
- `UnsupportedPrimitive` is removed because the semantic is understood and the failure is specifically geometric.

This does not imply support for flowchart primitives, images, metafiles, curves or layer references. Their existing typed diagnostics remain authoritative until separate Phase-2 work retires them with tested implementations.
