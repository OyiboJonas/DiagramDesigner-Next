// Compatibility entrypoint for architecture tests and future non-desktop consumers.
// The canonical browser implementation lives with the static Tauri frontend so
// every presentation backend can reuse the same document-space snapping semantics.
export {
  SnapContractError,
  buildRulerTicks,
  snapMoveDelta,
  visualBoundsMm,
} from "../../apps/desktop/ui/editor-interaction/snapping.mjs";
