# Phase 2 — straight connector outline rendering contract

## Scope

This contract promotes `LineStyle::Outline` for **straight connectors** from a typed SVG approximation to rendered legacy semantics. Orthogonal and curve connectors remain separate Phase-2 work because their geometry is not yet implemented by the production SVG facade.

Private/company `.ddd` examples are not part of this public contract and must remain local. The compatibility reference is the pinned public upstream Diagram Designer source.

## Public legacy reference

Pinned upstream source:

- repository: `meesoft/DiagramDesigner`;
- commit: `12188325704b559c211addf82f26183098b0e201`;
- file: `Application/LineObject.pas`;
- procedure: `TBaseConnectorObject.DrawLineStyle`;
- caller: `TStraightLineObject.Draw`.

For `lsOutline`, upstream stores the current pen width `I`, draws the complete connector once with the normal line pen, switches the pen colour to `FFillColor`, switches the pen width to `I div 2`, and draws the same connector again. It then restores the original width and line colour before endpoint markers are painted.

DiagramDesigner Next normalizes that contract to document millimetres rather than integer device pixels.

## SVG paint contract

For a visible straight connector with `LineStyle::Outline`:

1. the existing renderer line remains the **outer pass**, using the connector's normal primary stroke colour and full stroke width;
2. a second coincident line is emitted immediately after it as the **inner pass**, using `Connector.secondary_color` and exactly one half of the outer stroke width;
3. when no secondary colour is present, the legacy connector default is white;
4. RGBA opacity is preserved independently for primary and secondary colours;
5. system-palette colours continue to use the existing `#808080` fallback plus `SystemPaletteFallback` diagnostic;
6. the inner pass has `pointer-events="none"`, so it cannot replace the outer line as the editor interaction target;
7. connector rotation is repeated on the inner pass so both passes remain coincident after transformation.

An explicit element style with no stroke intentionally hides the connector. In that case no extra outline pass is materialized, but `LineStyle::Outline` is still considered understood rather than approximated.

## Marker ordering

The public upstream caller paints endpoint markers **after** `DrawLineStyle`. To preserve that ordering in SVG without moving the editor hit target away from the original rendered line, the facade emits an invisible marker carrier after the inner pass.

Standard marker attributes are attached to that carrier for outline connectors. The carrier has `stroke="none"` and `pointer-events="none"`; marker geometry itself retains the Phase-2 marker paint contract. Thus the DOM/render order is:

1. outer primary connector line;
2. inner secondary connector line;
3. invisible marker carrier with endpoint markers.

For non-outline straight connectors, marker attributes remain attached directly to the rendered connector line.

## Diagnostics

`ConnectorLineStyleApproximated { line_style: LineStyle::Outline }` is retired only when the facade successfully materializes the understood straight-connector outline semantic, including the explicit no-stroke case.

`LineStyle::Custom(_)` remains explicitly approximated. The facade does not guess arbitrary custom line-style codes.

Unsupported connector families remain separately typed through the existing primitive diagnostics until their geometry and paint contracts are implemented.

## Renderer boundary

All outline layering and marker-carrier state is renderer-specific and stays inside `render-svg`. No SVG DOM state is introduced into `next-domain`, `render-plan`, editor commands, history, persistence or DDNX.
