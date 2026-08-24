# Phase 2 public curve compatibility contract

This checkpoint defines DiagramDesigner Next's renderer contract for the four public
legacy `TCurveLineObject` families. It is derived from the pinned public upstream
reference `meesoft/DiagramDesigner` at commit
`12188325704b559c211addf82f26183098b0e201`, primarily
`Application/LineObject.pas` and `Application/DiagramBase.pas`.

The compatibility layer remains renderer-local. No SVG-specific state is added to
`next-domain`, editor history or semantic commands.

## Public type and persistence contract

Upstream declares:

```pascal
TCurveLineType = (ctCatmullRom, ctLegacy, ctBezier, ctLineSegments);
```

The curve type is serialized as one byte followed by a 16-bit point count and the
`TPoint` array. Files before version 16 default to `ctLegacy`. Files before version
28 deliberately bypass `TBaseConnectorObject.LoadFromStream`; version 28 and later
include connector metadata. Next therefore keeps the connector optional on
`ElementKind::Curve` rather than synthesizing connector semantics for old files.

## Curve body paint

`TCurveLineObject.Draw` forces the canvas pen to `psSolid` and then dispatches to
the selected curve geometry routine. It does **not** call
`TBaseConnectorObject.DrawLineStyle` for the curve body. Consequently, Next draws
the curve body as a solid stroke even when version-28 connector metadata contains a
non-solid `line_style` value. Applying dashes or an outline to the path would be a
modern approximation, not legacy-compatible behavior.

The normal line colour and width still apply. An explicit style without a stroke
hides the curve body.

## `ctLineSegments`

Upstream routes this family through `DrawAsPolyX`. The point buffer is padded to a
length of `CeilInt(Length(Points)-1, 3)+1` by repeating the final point, then drawn
with `Canvas.Polyline`. Repeated final points do not change the visible polyline, so
Next emits the original control points as one SVG path.

Marker direction uses `Points[1]` for the start and
`Points[Length(Points)-2]` for the end.

## `ctBezier`

`DrawAsPolyX` uses the same padding rule but sends the padded point array to Win32
`PolyBezier`. The first point starts the path; each subsequent group of three
points is one cubic Bezier segment. Next emits equivalent SVG cubic `C` segments
and repeats the final point until the public `1 mod 3` point-count contract is
satisfied.

Marker direction again uses the second original point and the penultimate original
point, not the padded duplicate points.

## `ctCatmullRom`

Upstream uses the uniform Catmull-Rom polynomial. Open splines duplicate the first
endpoint as the initial preceding control point and retain the final endpoint as
the last following control point. Closed splines are detected when the first and
last serialized points are byte-identical and wrap the control-point sequence.

Each segment is sampled using:

```text
floor(PointDist(P1, P2) / DesignerDPI * 50)
```

with a maximum of 1000 segments. `DiagramBase.pas` defines
`DesignerDPI = 64008` and `DesignerDPmm = 2520`, therefore the renderer-independent
millimetre form is:

```text
floor(distance_mm / 25.4 * 50)
```

capped at 1000. Next samples with this same rule rather than choosing a browser- or
zoom-dependent tessellation tolerance.

Two control points fall back to a straight line. For curves with more points,
marker direction follows the public derivative convention: the start direction is
`P1 + derivative(t=0.1)` and the end direction is
`P2 - derivative(t=0.9)` on the corresponding end sections.

## `ctLegacy`

The legacy spline keeps the public fixed `LineSegs = 32` blend tables. Next
recomputes the exact `FirstBlend`, `CenterBlend` and `LastBlend` polynomial weights
from `LineObject.pas` for each of those 32 samples.

The historical special cases are retained:

- three serialized points synthesize midpoints between points 0/1 and 1/2 for the
  four-point blend window;
- four serialized points synthesize the midpoint between points 1/2;
- larger point arrays slide the four-point blend window through the original
  points;
- two points fall back to a straight line.

Marker direction uses the second and penultimate serialized points.

## Connector markers

When version-aware migration provides connector metadata, standard endpoint markers
reuse the public Phase-2 marker geometry and paint contract. This includes the
legacy secondary-colour treatment used by outlined marker interiors. The marker
orientation comes from the curve-specific direction points above, not from a
straight line joining the curve's endpoints.

Unknown/custom marker codes remain explicit `ConnectorMarkerDeferred` diagnostics.
The stored line style is not treated as a deferred curve-body feature because the
public curve body itself ignores it.

## Invalid geometry

A curve needs at least two finite control points. Once the primitive family is
understood, malformed curve geometry is reported as `InvalidGeometry`; it is not
left behind as `UnsupportedPrimitive(Curve)`.

## Culling and `PreparedPage`

Bezier curves remain inside the convex hull of their control points, and line
segments remain inside their point bounds. Catmull-Rom and the legacy blend spline
can overshoot the serialized control-point rectangle, so `render-plan` computes
the same public sampled semantic geometry used by the renderer and conservatively
expands the serialized bounds to include any excursion. Standard marker clearance
is included when connector metadata is present.

The expansion is symmetric around the serialized rectangle before rotation. This
keeps rotation anchored to the same serialized object centre while guaranteeing
that cold `build_page_plan` and cached `PreparedPage` queries do not cull a visible
spline excursion.

## Compatibility boundary

This checkpoint retires the typed `Curve` unsupported diagnostic only for the four
public enum values already represented by `next-domain`. It does not infer new
curve families, reinterpret custom marker codes, or apply modern dashed/outlined
path styling that the public `TCurveLineObject.Draw` implementation does not use.
