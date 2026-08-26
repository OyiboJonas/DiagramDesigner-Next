from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: {label}: expected 1 match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


app = Path("crates/app-core/src/lib.rs")
replace_once(
    app,
    "    EditorSession, HistoryStateId, LayerScope, LayerTarget,\n    ResolvedPortPosition as CoreResolvedPortPosition,\n",
    "    EditorSession, HistoryStateId, LayerScope, LayerTarget,\n    ResolvedPortPosition as CoreResolvedPortPosition, ZOrderOperation as CoreZOrderOperation,\n",
    "import core z-order operation",
)
replace_once(
    app,
    "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ConnectorGeometryKind {\n    Straight,\n    Orthogonal,\n    Curve,\n}\n\n",
    "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ConnectorGeometryKind {\n    Straight,\n    Orthogonal,\n    Curve,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ZOrderOperation {\n    BringToFront,\n    SendToBack,\n    BringForward,\n    SendBackward,\n}\n\nimpl From<ZOrderOperation> for CoreZOrderOperation {\n    fn from(value: ZOrderOperation) -> Self {\n        match value {\n            ZOrderOperation::BringToFront => Self::BringToFront,\n            ZOrderOperation::SendToBack => Self::SendToBack,\n            ZOrderOperation::BringForward => Self::BringForward,\n            ZOrderOperation::SendBackward => Self::SendBackward,\n        }\n    }\n}\n\n",
    "add application z-order enum",
)
replace_once(
    app,
    "    /// Create one element through the editor-core semantic command boundary.\n    pub fn create_element(\n",
    "    /// Reorder top-level elements through editor-core's canonical scene-root order.\n    pub fn reorder_elements(\n        &mut self,\n        element_ids: Vec<ElementId>,\n        operation: ZOrderOperation,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::ReorderElements {\n            element_ids,\n            operation: operation.into(),\n        })\n    }\n\n    /// Create one element through the editor-core semantic command boundary.\n    pub fn create_element(\n",
    "add application reorder method",
)


desktop = Path("apps/desktop/src-tauri/src/lib.rs")
replace_once(
    desktop,
    "    ConnectorEndpoints as AppConnectorEndpoints, ConnectorGeometryKind as AppConnectorGeometryKind,\n    ElementAppearanceUpdate,\n",
    "    ConnectorEndpoints as AppConnectorEndpoints, ConnectorGeometryKind as AppConnectorGeometryKind,\n    ElementAppearanceUpdate, ZOrderOperation as AppZOrderOperation,\n",
    "import application z-order operation",
)
replace_once(
    desktop,
    "#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\nenum BasicElementKind {\n",
    "#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nenum ZOrderOperationRequest {\n    BringToFront,\n    SendToBack,\n    BringForward,\n    SendBackward,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct ReorderSelectionRequest {\n    operation: ZOrderOperationRequest,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\nenum BasicElementKind {\n",
    "add desktop request types",
)

command = '''#[tauri::command]
fn reorder_selection(
    request: ReorderSelectionRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let selected: Vec<_> = document
        .session
        .session()
        .selection()
        .iter()
        .copied()
        .collect();
    if selected.is_empty() {
        return Ok(element_edit_result_dto(&document));
    }

    {
        let session = document.session.session();
        let page_id = session.active_page_id().ok_or_else(|| {
            CommandError::new(
                "arrange_no_active_page",
                "Choose an active page before arranging elements.",
            )
        })?;
        let layer_id = document.session.active_page_layer_id().ok_or_else(|| {
            CommandError::new(
                "arrange_no_active_layer",
                "Choose a page-local layer before arranging elements.",
            )
        })?;
        let page = session
            .document()
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .ok_or_else(|| {
                CommandError::new(
                    "arrange_page_missing",
                    "The active page no longer exists.",
                )
            })?;
        let layer = page
            .layers
            .iter()
            .find(|layer| layer.id == layer_id)
            .ok_or_else(|| {
                CommandError::new(
                    "arrange_layer_missing",
                    "The active layer no longer exists.",
                )
            })?;
        if !layer.visible {
            return Err(CommandError::new(
                "arrange_layer_hidden",
                "Elements can be arranged only on a visible active layer.",
            ));
        }
        if layer.locked {
            return Err(CommandError::new(
                "arrange_layer_locked",
                "Unlock the active layer before arranging elements.",
            ));
        }
        if selected.iter().any(|element_id| {
            !layer
                .scene
                .elements
                .iter()
                .any(|element| element.id == *element_id)
        }) {
            return Err(CommandError::new(
                "arrange_not_on_active_layer",
                "Every selected element must belong to the active layer.",
            ));
        }
    }

    let operation = match request.operation {
        ZOrderOperationRequest::BringToFront => AppZOrderOperation::BringToFront,
        ZOrderOperationRequest::SendToBack => AppZOrderOperation::SendToBack,
        ZOrderOperationRequest::BringForward => AppZOrderOperation::BringForward,
        ZOrderOperationRequest::SendBackward => AppZOrderOperation::SendBackward,
    };
    document
        .session
        .reorder_elements(selected, operation)
        .map_err(|error| CommandError::new("arrange_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

'''
replace_once(
    desktop,
    "#[tauri::command]\nfn copy_selection(state: State<'_, DesktopState>) -> Result<ClipboardCopyDto, CommandError> {\n",
    command
    + "#[tauri::command]\nfn copy_selection(state: State<'_, DesktopState>) -> Result<ClipboardCopyDto, CommandError> {\n",
    "add reorder command",
)
replace_once(
    desktop,
    "            selection_properties,\n            copy_selection,\n",
    "            selection_properties,\n            reorder_selection,\n            copy_selection,\n",
    "register reorder command",
)
