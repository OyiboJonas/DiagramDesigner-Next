# Phase 2 flowchart contract

## Public reference

DiagramDesigner Next derives the standard flowchart family from the pinned public upstream `Application/FlowchartObject.pas` at commit `12188325704b559c211addf82f26183098b0e201`.

The public legacy codes are:

| Legacy code | Upstream name | Next shape key |
| --- | --- | --- |
| `0x11` / 17 | Side bars | `builtin:diagramdesigner-flowchart/17` |
| `0x21` / 33 | Rounded 1 | `builtin:diagramdesigner-flowchart/33` |
| `0x22` / 34 | Rounded 2 | `builtin:diagramdesigner-flowchart/34` |
| `0x23` / 35 | Rounded 3 | `builtin:diagramdesigner-flowchart/35` |
| `0x31` / 49 | Slant right | `builtin:diagramdesigner-flowchart/49` |
| `0x32` / 50 | Slant left | `builtin:diagramdesigner-flowchart/50` |
| `0x41` / 65 | Odd rounded 1 | `builtin:diagramdesigner-flowchart/65` |
| `0x51` / 81 | Odd rounded 2 | `builtin:diagramdesigner-flowchart/81` |

Unknown/custom values are not approximated as rectangles. They retain the typed `UnsupportedPrimitive(Flowchart)` diagnostic.

## Geometry

The renderer follows the public `TFlowchartObject.Draw` construction rather than introducing independent Next-only shapes.

- **Side bars**: the outer object rectangle plus vertical bars at one eighth and seven eighths of its width.
- **Rounded 1**: Win32 round-rectangle corner ellipse diameter equals the object height, represented by an SVG corner radius of half the height.
- **Rounded 2**: corner radius is one quarter of the object height.
- **Rounded 3**: corner radius is one eighth of the object height.
- **Slant right / left**: four-point parallelograms using a horizontal offset of one eighth of the object height. Their geometry extends beyond both horizontal sides of the serialized object rectangle.
- **Odd rounded 1**: 32-point polygon using the public sine/cosine construction; its right side uses half of the left-side horizontal curvature.
- **Odd rounded 2**: 32-point polygon using the same public construction, with the right-hand curve extending outside the serialized rectangle by up to half the object height.

The renderer uses document-space floating-point geometry rather than reproducing device-pixel rounding. This keeps the geometry scale independent while preserving the public mathematical construction.

## Paint

Flowcharts inherit the imported `TShapeObject` stroke, fill and gradient semantics. RGBA colours are emitted directly. Unresolved system palette colours retain the existing renderer diagnostic and platform-independent fallback. The side-bar interior lines use the same stroke as the outer shape.

## Culling

The render-plan culling boundary includes public geometry excursions:

- slant right / left: expand both horizontal sides by `height / 8`;
- odd rounded 2: expand the right side by `height / 2`.

This expansion is renderer-independent and shared by cold `build_page_plan` and `PreparedPage`, preventing visible shape excursions from being culled solely because the viewport does not intersect the serialized object rectangle.

## Compatibility boundary

This checkpoint retires the Flowchart unsupported-primitive diagnostic only for the eight public standard codes above. It does not define semantics for unknown/custom flowchart codes and does not alter `next-domain` shape identity or editor history.
