# Phase 2 inherited-layer / layer-reference contract

## Scope

This document defines DiagramDesigner Next's renderer compatibility boundary for the public legacy `TInheritedLayerObject` (`otInheritedLayer`) implementation pinned at upstream commit `12188325704b559c211addf82f26183098b0e201`.

The Next domain already represents this semantic as:

```rust
ElementKind::LayerReference {
    relative_page_index: i32,
    layer_index: i32,
}
```

No renderer-specific target IDs, cached scene fragments or SVG state are persisted in `next-domain` or editor history.

## Public upstream behaviour

The pinned `Application/PictureObject.pas` implementation stores two 32-bit integers:

- `FRelativePageIndex`
- `FLayerIndex`

During drawing it:

1. copies the current `CanvasInfo`;
2. adds `FRelativePageIndex` to the current page index;
3. verifies that the resulting page index exists;
4. verifies that `FLayerIndex` exists on that page;
5. sets a per-object `Drawing` guard to prevent recursive re-entry;
6. disables Z-buffer drawing for the referenced layer;
7. adds the inherited-layer object's left/top position to the drawing offset;
8. multiplies X scale by `object.Width / targetPage.Width`;
9. multiplies Y scale by `object.Height / targetPage.Height`;
10. draws exactly `targetPage.Layers[FLayerIndex]`;
11. clears the recursion guard.

Editing/drag mode is changed to preview mode for the nested layer. The render and shadow paths apply the same page/layer resolution and position/scale transform.

## Next SVG materialization

The Phase-2 SVG layer resolves a reference from the page currently being rendered, not from the page on which the target layer was originally created. This is required for nested relative references.

A valid target is emitted as a nested SVG viewport:

- `x` / `y` = normalized reference-object left/top;
- `width` / `height` = reference-object width/height;
- `viewBox = "0 0 targetPage.width targetPage.height"`;
- `preserveAspectRatio = "none"` to preserve the legacy independent X/Y scale factors;
- `overflow = "visible"` so the compatibility layer does not invent clipping absent from the upstream draw path.

Only the selected page layer is planned inside that nested SVG. Master layers and sibling page layers are not inherited.

The target layer is planned directly rather than routed through normal page visibility filtering. This matches the upstream object's direct `Page.Layers[FLayerIndex].Draw(...)` call and also keeps imported legacy behaviour independent of Next-only layer visibility state.

## Nesting and recursion

Nested layer references are supported. Each nested reference resolves its `relative_page_index` from the target page currently being rendered.

The renderer maintains a stack of layer-reference element IDs. If the same reference object is re-entered, its nested invocation is not materialized. This mirrors the public per-instance `Drawing` guard and terminates direct as well as multi-reference cycles once an already-active reference is reached.

The selected core renderer's typed `UnsupportedPrimitive { family: LayerReference }` diagnostic is retired only for a reference invocation that is actually resolved and materialized. Invalid page/layer indices, malformed geometry, and recursion-suppressed nested invocations retain explicit unsupported diagnostics; no placeholder rectangle or guessed target is drawn.

## Paint, definition isolation and z-order

A layer reference has no independent fill/stroke paint contract. Its visual result is the referenced layer content.

The same target layer can be referenced multiple times. Referenced SVG can therefore contain repeated definition IDs for gradients, markers or other `<defs>` resources derived from the same source element IDs. Before a referenced SVG is embedded, renderer-local definition IDs and their `url(#...)` / fragment references are namespaced by the current layer-reference element ID. Nested references apply the same rule recursively, so sibling or repeated references cannot collide while stable domain `ElementId` values remain unchanged.

The reference fragment occupies the original reference element's position in the current render-plan order. Content within the referenced layer preserves that layer's root/group traversal order and uses the same production SVG compatibility pipeline as top-level content, including curves, flowcharts, raster images, connector semantics and optional metafile renditions.

## Culling boundary

The renderer-independent planner continues to use the serialized layer-reference object rectangle as the reference's culling boundary. This is the natural public object boundary and preserves identical cold-plan and `PreparedPage` selection for normal page-contained inherited-layer content.

The nested SVG is intentionally not clipped. Therefore a target-layer object deliberately positioned outside its source page can visually overflow the reference rectangle when culling is disabled. Phase 2 does not silently expand the persisted/domain bounds from renderer recursion; if public compatibility evidence later requires viewport selection of such out-of-page inherited content, that can be added as a renderer-independent semantic-bounds extension without changing the domain model.

## Public verification

Synthetic renderer tests cover:

- backward relative page references;
- forward relative page references;
- independent X/Y page-to-reference scaling;
- target-layer content and z-order;
- direct target-layer rendering independent of the Next-only page visibility flag;
- invalid target page indices remaining typed unsupported;
- nested relative references resolving from each target page;
- recursion suppression without placeholder rendering;
- repeated references receiving isolated SVG definition IDs without changing source element IDs;
- cold `render-plan` versus `PreparedPage` viewport equivalence.

No private `.ddd` fixtures or private-corpus metadata are required for this contract.
