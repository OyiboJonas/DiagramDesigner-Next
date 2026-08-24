from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


# Desktop does not name LayerTarget directly; keep editor-core behind app-core.
desktop_path = Path("apps/desktop/src-tauri/src/lib.rs")
desktop = desktop_path.read_text(encoding="utf-8")
desktop = replace_once(
    desktop,
    "use editor_core::LayerTarget;\n",
    "",
    "remove transitive LayerTarget import",
)
desktop_path.write_text(desktop, encoding="utf-8")

# Keep the move-specific documentation attached to the public move method.
app_path = Path("crates/app-core/src/lib.rs")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    '''    /// Commit the final document-space delta from a completed frontend move
    /// gesture. Raw pointer updates must never call this method.
    fn execute_edit(&mut self, command: EditCommand) -> Result<bool, ApplicationError> {''',
    '''    fn execute_edit(&mut self, command: EditCommand) -> Result<bool, ApplicationError> {''',
    "detach move docs from helper",
)
app = replace_once(
    app,
    '''    pub fn commit_move_elements(
        &mut self,''',
    '''    /// Commit the final document-space delta from a completed frontend move
    /// gesture. Raw pointer updates must never call this method.
    pub fn commit_move_elements(
        &mut self,''',
    "reattach move docs",
)
app_path.write_text(app, encoding="utf-8")

# Busy-state release must not accidentally re-enable actions that require selection.
js_path = Path("apps/desktop/ui/app.js")
js = js_path.read_text(encoding="utf-8")
js = replace_once(
    js,
    '''function setBusy(busy) {
  for (const button of actionButtons) {
    button.disabled = busy;
  }
}''',
    '''function setBusy(busy) {
  for (const button of actionButtons) {
    button.disabled = busy;
  }
  if (!busy) {
    const selectionCount = Number(currentSelectionProperties?.count ?? 0);
    elements.deleteSelection.disabled = selectionCount === 0;
    elements.applyProperties.disabled = !currentSelectionProperties?.primary;
  }
}''',
    "selection-aware busy release",
)
js_path.write_text(js, encoding="utf-8")

print("Applied first-editor cleanup")
