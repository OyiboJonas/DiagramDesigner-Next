// Compatibility entrypoint for architecture tests and future non-desktop consumers.
// The canonical browser implementation lives with the static Tauri frontend so
// the desktop shell executes the exact same pointer lifecycle that CI tests.
export {
  InteractionContractError,
  MoveGestureController,
  bindMovePointerSurface,
} from "../../apps/desktop/ui/editor-interaction/move-gesture.mjs";
