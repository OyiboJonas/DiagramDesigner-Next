from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    text = p.read_text(encoding='utf-8')
    if text.count(old) != 1:
        raise RuntimeError(f'{path}: expected exactly one match, got {text.count(old)}')
    p.write_text(text.replace(old, new, 1), encoding='utf-8')

replace_once(
    'crates/app-core/src/lib.rs',
    '''#[derive(Debug, Clone, Copy, PartialEq)]\npub struct ConnectorPortPosition {\n    pub element_id: ElementId,\n    pub port_id: PortId,\n    pub position_mm: Point,\n}\n''',
    '''#[derive(Debug, Clone, Copy, PartialEq)]\npub struct ConnectorPortPosition {\n    pub element_id: ElementId,\n    pub port_id: PortId,\n    pub position_mm: Point,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct ElementAppearanceUpdate {\n    pub element_id: ElementId,\n    pub stroke: Option<StrokeStyle>,\n    pub fill: Option<FillStyle>,\n    pub text_color: Option<Color>,\n}\n''',
)

replace_once(
    'crates/app-core/src/lib.rs',
    '''    pub fn create_elements(\n        &mut self,\n        target: LayerTarget,\n        elements: Vec<Element>,\n    ) -> Result<bool, ApplicationError> {\n        let commands = elements\n            .into_iter()\n            .map(|element| EditCommand::CreateElement {\n                target,\n                element,\n                z_index: None,\n            });\n        self.execute_edit_transaction(EditTransaction::new(commands))\n    }\n''',
    '''    pub fn create_elements(\n        &mut self,\n        target: LayerTarget,\n        elements: Vec<Element>,\n        appearance_updates: Vec<ElementAppearanceUpdate>,\n    ) -> Result<bool, ApplicationError> {\n        let mut transaction = EditTransaction::new(elements.into_iter().map(|element| {\n            EditCommand::CreateElement {\n                target,\n                element,\n                z_index: None,\n            }\n        }));\n        for update in appearance_updates {\n            transaction.push(EditCommand::SetElementAppearance {\n                element_id: update.element_id,\n                stroke: update.stroke,\n                fill: update.fill,\n                text_color: update.text_color,\n            });\n        }\n        self.execute_edit_transaction(transaction)\n    }\n''',
)

replace_once(
    'apps/desktop/src-tauri/src/clipboard.rs',
    '''pub struct ClipboardInstantiation {\n    pub elements: Vec<Element>,\n    pub element_ids: Vec<ElementId>,\n}\n''',
    '''pub struct ClipboardInstantiation {\n    pub elements: Vec<Element>,\n    pub element_ids: Vec<ElementId>,\n    pub source_element_ids: BTreeMap<ElementId, ElementId>,\n}\n''',
)

replace_once(
    'apps/desktop/src-tauri/src/clipboard.rs',
    '''        ClipboardInstantiation {\n            elements: instantiated,\n            element_ids: selected,\n        }\n''',
    '''        ClipboardInstantiation {\n            elements: instantiated,\n            element_ids: selected,\n            source_element_ids: element_ids,\n        }\n''',
)

replace_once(
    'apps/desktop/src-tauri/src/lib.rs',
    '''use app_core::{\n    ApplicationSession, ConnectorEndpointSide as AppConnectorEndpointSide,\n    ConnectorEndpointState as AppConnectorEndpointState,\n    ConnectorEndpoints as AppConnectorEndpoints, ConnectorGeometryKind as AppConnectorGeometryKind,\n};\n''',
    '''use app_core::{\n    ApplicationSession, ConnectorEndpointSide as AppConnectorEndpointSide,\n    ConnectorEndpointState as AppConnectorEndpointState,\n    ConnectorEndpoints as AppConnectorEndpoints, ConnectorGeometryKind as AppConnectorGeometryKind,\n    ElementAppearanceUpdate,\n};\n''',
)

replace_once(
    'apps/desktop/src-tauri/src/lib.rs',
    '''    MarkerStyle, NextArtifact, NormalizedPoint, Page, PageId, Point, Port, PortId, Rect,\n    RichTextDocument, RichTextToken, Scene, Size, StrokeStyle, TextBlock, TextHorizontalAlignment,\n''',
    '''    MarkerStyle, NextArtifact, NormalizedPoint, Page, PageId, Point, Port, PortId, Rect,\n    RichTextDocument, RichTextToken, Scene, Size, StrokeStyle, StyleId, TextBlock,\n    TextHorizontalAlignment,\n''',
)

replace_once(
    'apps/desktop/src-tauri/src/lib.rs',
    '''    let instantiated = clipboard.payload.instantiate(next_step);\n    let selected = instantiated.element_ids.clone();\n    document\n        .session\n        .create_elements(target, instantiated.elements)\n        .map_err(|error| CommandError::new("clipboard_paste_failed", error.to_string()))?;\n''',
    '''    let mut instantiated = clipboard.payload.instantiate(next_step);\n    let selected = instantiated.element_ids.clone();\n    let appearance_updates = prepare_clipboard_appearance_updates(\n        document.session.session().document(),\n        &mut instantiated,\n    )?;\n    document\n        .session\n        .create_elements(target, instantiated.elements, appearance_updates)\n        .map_err(|error| CommandError::new("clipboard_paste_failed", error.to_string()))?;\n''',
)

replace_once(
    'apps/desktop/src-tauri/src/lib.rs',
    '''    let instantiated = payload.instantiate(1);\n    let duplicated_ids = instantiated.element_ids.clone();\n    document\n        .session\n        .create_elements(target, instantiated.elements)\n        .map_err(|error| CommandError::new("duplicate_failed", error.to_string()))?;\n''',
    '''    let mut instantiated = payload.instantiate(1);\n    let duplicated_ids = instantiated.element_ids.clone();\n    let appearance_updates = prepare_clipboard_appearance_updates(\n        document.session.session().document(),\n        &mut instantiated,\n    )?;\n    document\n        .session\n        .create_elements(target, instantiated.elements, appearance_updates)\n        .map_err(|error| CommandError::new("duplicate_failed", error.to_string()))?;\n''',
)

replace_once(
    'apps/desktop/src-tauri/src/lib.rs',
    '''fn clear_application_clipboard(state: &State<'_, DesktopState>) -> Result<(), CommandError> {\n    let mut application_clipboard = state.clipboard.lock().map_err(|_| {\n        CommandError::new(\n            "clipboard_lock_failed",\n            "The application clipboard lock is poisoned.",\n        )\n    })?;\n    *application_clipboard = None;\n    Ok(())\n}\n''',
    '''fn clear_application_clipboard(state: &State<'_, DesktopState>) -> Result<(), CommandError> {\n    let mut application_clipboard = state.clipboard.lock().map_err(|_| {\n        CommandError::new(\n            "clipboard_lock_failed",\n            "The application clipboard lock is poisoned.",\n        )\n    })?;\n    *application_clipboard = None;\n    Ok(())\n}\n\nfn prepare_clipboard_appearance_updates(\n    document: &Document,\n    instantiated: &mut clipboard::ClipboardInstantiation,\n) -> Result<Vec<ElementAppearanceUpdate>, CommandError> {\n    const APPEARANCE_STYLE_NAMESPACE: &str = "diagramdesigner-next:element-appearance";\n    let mut updates = Vec::new();\n\n    for (source_id, copied_id) in &instantiated.source_element_ids {\n        let source = find_element(document, *source_id).ok_or_else(|| {\n            CommandError::new(\n                "clipboard_source_missing",\n                "A copied source element no longer exists in the current document.",\n            )\n        })?;\n        let dedicated_style_id = StyleId::v5(source_id.0, APPEARANCE_STYLE_NAMESPACE);\n        if source.style_id != Some(dedicated_style_id) {\n            continue;\n        }\n        let style = document\n            .styles\n            .iter()\n            .find(|style| style.id == dedicated_style_id)\n            .ok_or_else(|| {\n                CommandError::new(\n                    "clipboard_appearance_missing",\n                    "The copied element's dedicated appearance style is missing.",\n                )\n            })?;\n        let copied = instantiated\n            .elements\n            .iter_mut()\n            .find(|element| element.id == *copied_id)\n            .ok_or_else(|| {\n                CommandError::new(\n                    "clipboard_copy_missing",\n                    "The instantiated clipboard element could not be resolved.",\n                )\n            })?;\n\n        // A dedicated appearance style is element-owned. Do not share the source\n        // element's deterministic style ID with the copy: create the copied element\n        // unstyled first, then materialize an equivalent dedicated style in the same\n        // editor transaction under the copied element's own deterministic ID.\n        copied.style_id = None;\n        updates.push(ElementAppearanceUpdate {\n            element_id: *copied_id,\n            stroke: style.stroke.clone(),\n            fill: style.fill.clone(),\n            text_color: style.text_color,\n        });\n    }\n\n    Ok(updates)\n}\n''',
)
