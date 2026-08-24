from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"Anchor not found in {path}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


def append_text(path: str, extra: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    if extra.strip() in text:
        return
    file.write_text(text.rstrip() + "\n\n" + extra.strip() + "\n", encoding="utf-8")


# --- app-core: expose page/layer semantics without leaking editor-core targets to desktop ---
replace_once(
    "crates/app-core/src/lib.rs",
    """use editor_core::{\n    EditCommand, EditTransaction, EditorError, EditorSession, HistoryStateId, LayerTarget,\n};\nuse editor_runtime::{EditorRuntime, RecoveryCheckpointKey, RecoveryPlan};\nuse next_domain::{Element, ElementId, NextArtifact, Point, Rect, TextBlock};\n""",
    """use editor_core::{\n    EditCommand, EditTransaction, EditorError, EditorSession, HistoryStateId, LayerScope,\n    LayerTarget,\n};\nuse editor_runtime::{EditorRuntime, RecoveryCheckpointKey, RecoveryPlan};\nuse next_domain::{\n    Color, Element, ElementId, Layer, LayerId, NextArtifact, Page, PageId, Point, Rect, Size,\n    TextBlock,\n};\n""",
)

replace_once(
    "crates/app-core/src/lib.rs",
    """        self.execute_edit_transaction(transaction)\n    }\n\n    pub fn set_selection<I>(&mut self, element_ids: I) -> Result<(), ApplicationError>\n""",
    """        self.execute_edit_transaction(transaction)\n    }\n\n    /// Switch the active page without creating a persistent history step.\n    pub fn set_active_page(&mut self, page_id: PageId) -> Result<(), ApplicationError> {\n        self.runtime.session_mut().set_active_page(page_id)?;\n        Ok(())\n    }\n\n    /// Return the active page-local layer ID while keeping `LayerTarget` private to app-core.\n    pub fn active_page_layer_id(&self) -> Option<LayerId> {\n        match self.runtime.session().active_layer()? {\n            LayerTarget::Page { layer_id, .. } => Some(layer_id),\n            LayerTarget::Master { .. } => None,\n        }\n    }\n\n    /// Switch the active page-local layer without creating a persistent history step.\n    pub fn set_active_page_layer(\n        &mut self,\n        page_id: PageId,\n        layer_id: LayerId,\n    ) -> Result<(), ApplicationError> {\n        self.runtime\n            .session_mut()\n            .set_active_layer(LayerTarget::Page { page_id, layer_id })?;\n        Ok(())\n    }\n\n    pub fn create_page(&mut self, page: Page) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::CreatePage { page, index: None })\n    }\n\n    pub fn delete_page(&mut self, page_id: PageId) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::DeletePage { page_id })\n    }\n\n    pub fn set_page_properties(\n        &mut self,\n        page_id: PageId,\n        name: String,\n        size_mm: Size,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::SetPageProperties {\n            page_id,\n            name,\n            size_mm,\n        })\n    }\n\n    pub fn create_page_layer(\n        &mut self,\n        page_id: PageId,\n        layer: Layer,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::CreateLayer {\n            scope: LayerScope::Page { page_id },\n            layer,\n            index: None,\n        })\n    }\n\n    pub fn delete_page_layer(\n        &mut self,\n        page_id: PageId,\n        layer_id: LayerId,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::DeleteLayer {\n            target: LayerTarget::Page { page_id, layer_id },\n        })\n    }\n\n    pub fn set_page_layer_properties(\n        &mut self,\n        page_id: PageId,\n        layer_id: LayerId,\n        name: String,\n        visible: bool,\n        locked: bool,\n        draw_color: Option<Color>,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::SetLayerProperties {\n            target: LayerTarget::Page { page_id, layer_id },\n            name,\n            visible,\n            locked,\n            draw_color,\n        })\n    }\n\n    pub fn set_selection<I>(&mut self, element_ids: I) -> Result<(), ApplicationError>\n""",
)

replace_once(
    "crates/app-core/src/lib.rs",
    """        assert_eq!(app.session().current_history_state(), initial);\n        assert!(!app.is_dirty());\n    }\n}\n""",
    """        assert_eq!(app.session().current_history_state(), initial);\n        assert!(!app.is_dirty());\n    }\n\n    #[test]\n    fn page_and_layer_commands_keep_navigation_transient_and_structure_in_history() {\n        let (artifact, _) = fixture();\n        let mut app = ApplicationSession::from_artifact(artifact).unwrap();\n        let first_page = app.session().active_page_id().unwrap();\n        let first_layer = app.active_page_layer_id().unwrap();\n        let initial = app.session().current_history_state();\n\n        let second_page = PageId::new();\n        let second_layer = LayerId::new();\n        assert!(\n            app.create_page(Page {\n                id: second_page,\n                name: \"Page 2\".to_owned(),\n                size_mm: Size {\n                    width: 297.0,\n                    height: 210.0,\n                },\n                layers: vec![Layer {\n                    id: second_layer,\n                    name: \"Layer 1\".to_owned(),\n                    visible: true,\n                    locked: false,\n                    draw_color: None,\n                    scene: Scene::default(),\n                }],\n            })\n            .unwrap()\n        );\n        let after_page_create = app.session().current_history_state();\n        assert_ne!(after_page_create, initial);\n\n        app.set_active_page(second_page).unwrap();\n        app.set_active_page_layer(second_page, second_layer).unwrap();\n        assert_eq!(app.session().active_page_id(), Some(second_page));\n        assert_eq!(app.active_page_layer_id(), Some(second_layer));\n        assert_eq!(app.session().current_history_state(), after_page_create);\n\n        let extra_layer = LayerId::new();\n        assert!(\n            app.create_page_layer(\n                second_page,\n                Layer {\n                    id: extra_layer,\n                    name: \"Layer 2\".to_owned(),\n                    visible: true,\n                    locked: false,\n                    draw_color: None,\n                    scene: Scene::default(),\n                },\n            )\n            .unwrap()\n        );\n        app.set_active_page_layer(second_page, extra_layer).unwrap();\n        let after_layer_create = app.session().current_history_state();\n        assert_eq!(app.active_page_layer_id(), Some(extra_layer));\n\n        assert!(\n            app.set_page_properties(\n                second_page,\n                \"Landscape\".to_owned(),\n                Size {\n                    width: 320.0,\n                    height: 180.0,\n                },\n            )\n            .unwrap()\n        );\n        assert!(\n            app.set_page_layer_properties(\n                second_page,\n                extra_layer,\n                \"Annotations\".to_owned(),\n                false,\n                true,\n                None,\n            )\n            .unwrap()\n        );\n        let page = app\n            .session()\n            .document()\n            .pages\n            .iter()\n            .find(|page| page.id == second_page)\n            .unwrap();\n        assert_eq!(page.name, \"Landscape\");\n        let layer = page\n            .layers\n            .iter()\n            .find(|layer| layer.id == extra_layer)\n            .unwrap();\n        assert_eq!(layer.name, \"Annotations\");\n        assert!(!layer.visible);\n        assert!(layer.locked);\n\n        assert!(app.delete_page_layer(second_page, extra_layer).unwrap());\n        assert!(app.undo().unwrap());\n        assert!(app\n            .session()\n            .document()\n            .pages\n            .iter()\n            .find(|page| page.id == second_page)\n            .unwrap()\n            .layers\n            .iter()\n            .any(|layer| layer.id == extra_layer));\n\n        app.set_active_page(first_page).unwrap();\n        app.set_active_page_layer(first_page, first_layer).unwrap();\n        let before_page_delete = app.session().current_history_state();\n        assert!(app.delete_page(second_page).unwrap());\n        assert_eq!(app.session().document().pages.len(), 1);\n        assert!(app.undo().unwrap());\n        assert_eq!(app.session().document().pages.len(), 2);\n        assert_eq!(app.session().current_history_state(), before_page_delete);\n    }\n}\n""",
)


# --- desktop Rust: DTOs, commands, policies and helpers ---
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """struct ElementPropertiesDto {\n    element_id: ElementId,\n    name: String,\n    element_type: &'static str,\n    bounds_mm: Rect,\n    rotation_deg: f64,\n    text: Option<String>,\n    text_editable: bool,\n}\n\n#[tauri::command]\nfn document_state""",
    """struct ElementPropertiesDto {\n    element_id: ElementId,\n    name: String,\n    element_type: &'static str,\n    bounds_mm: Rect,\n    rotation_deg: f64,\n    text: Option<String>,\n    text_editable: bool,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct PageIdRequest {\n    page_id: PageId,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct LayerIdRequest {\n    page_id: PageId,\n    layer_id: LayerId,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct UpdatePagePropertiesRequest {\n    page_id: PageId,\n    name: String,\n    size_mm: Size,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct UpdateLayerPropertiesRequest {\n    page_id: PageId,\n    layer_id: LayerId,\n    name: String,\n    visible: bool,\n    locked: bool,\n}\n\n#[derive(Debug, Serialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct DocumentNavigationDto {\n    state: DocumentStateDto,\n    pages: Vec<PageNavigationDto>,\n    active_page_id: Option<PageId>,\n    active_layer_id: Option<LayerId>,\n}\n\n#[derive(Debug, Serialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct PageNavigationDto {\n    page_id: PageId,\n    name: String,\n    size_mm: Size,\n    layers: Vec<LayerNavigationDto>,\n}\n\n#[derive(Debug, Serialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct LayerNavigationDto {\n    layer_id: LayerId,\n    name: String,\n    visible: bool,\n    locked: bool,\n    element_count: usize,\n}\n\n#[tauri::command]\nfn document_state""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """fn document_state(state: State<'_, DesktopState>) -> Result<DocumentStateDto, CommandError> {\n    let document = lock_document(&state)?;\n    Ok(document_state_dto(&document))\n}\n\n#[tauri::command]\nasync fn open_renderer_benchmark""",
    """fn document_state(state: State<'_, DesktopState>) -> Result<DocumentStateDto, CommandError> {\n    let document = lock_document(&state)?;\n    Ok(document_state_dto(&document))\n}\n\n#[tauri::command]\nfn document_navigation(\n    state: State<'_, DesktopState>,\n) -> Result<DocumentNavigationDto, CommandError> {\n    let document = lock_document(&state)?;\n    Ok(document_navigation_dto(&document))\n}\n\n#[tauri::command]\nfn activate_page(\n    request: PageIdRequest,\n    state: State<'_, DesktopState>,\n) -> Result<DocumentNavigationDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    document\n        .session\n        .set_active_page(request.page_id)\n        .map_err(|error| CommandError::new(\"activate_page_failed\", error.to_string()))?;\n    document.session.clear_selection();\n    Ok(document_navigation_dto(&document))\n}\n\n#[tauri::command]\nfn activate_layer(\n    request: LayerIdRequest,\n    state: State<'_, DesktopState>,\n) -> Result<DocumentNavigationDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    document\n        .session\n        .set_active_page_layer(request.page_id, request.layer_id)\n        .map_err(|error| CommandError::new(\"activate_layer_failed\", error.to_string()))?;\n    document.session.clear_selection();\n    Ok(document_navigation_dto(&document))\n}\n\n#[tauri::command]\nfn create_page(\n    state: State<'_, DesktopState>,\n) -> Result<DocumentNavigationDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let (number, size_mm) = {\n        let session = document.session.session();\n        let size_mm = session\n            .active_page_id()\n            .and_then(|page_id| {\n                session\n                    .document()\n                    .pages\n                    .iter()\n                    .find(|page| page.id == page_id)\n                    .map(|page| page.size_mm)\n            })\n            .unwrap_or(Size {\n                width: 210.0,\n                height: 297.0,\n            });\n        (session.document().pages.len() + 1, size_mm)\n    };\n    let page = empty_page(format!(\"Page {number}\"), size_mm);\n    let page_id = page.id;\n    let layer_id = page.layers[0].id;\n    document\n        .session\n        .create_page(page)\n        .map_err(|error| CommandError::new(\"page_create_failed\", error.to_string()))?;\n    document\n        .session\n        .set_active_page(page_id)\n        .map_err(|error| CommandError::new(\"activate_page_failed\", error.to_string()))?;\n    document\n        .session\n        .set_active_page_layer(page_id, layer_id)\n        .map_err(|error| CommandError::new(\"activate_layer_failed\", error.to_string()))?;\n    document.session.clear_selection();\n    Ok(document_navigation_dto(&document))\n}\n\n#[tauri::command]\nfn delete_page(\n    request: PageIdRequest,\n    state: State<'_, DesktopState>,\n) -> Result<DocumentNavigationDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    if document.session.session().document().pages.len() <= 1 {\n        return Err(CommandError::new(\n            \"last_page_delete_blocked\",\n            \"A document must keep at least one page.\",\n        ));\n    }\n    document\n        .session\n        .delete_page(request.page_id)\n        .map_err(|error| CommandError::new(\"page_delete_failed\", error.to_string()))?;\n    document.session.clear_selection();\n    Ok(document_navigation_dto(&document))\n}\n\n#[tauri::command]\nfn update_page_properties(\n    request: UpdatePagePropertiesRequest,\n    state: State<'_, DesktopState>,\n) -> Result<DocumentNavigationDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let name = structure_name(&request.name)?;\n    document\n        .session\n        .set_page_properties(request.page_id, name, request.size_mm)\n        .map_err(|error| CommandError::new(\"page_properties_failed\", error.to_string()))?;\n    Ok(document_navigation_dto(&document))\n}\n\n#[tauri::command]\nfn create_layer(\n    request: PageIdRequest,\n    state: State<'_, DesktopState>,\n) -> Result<DocumentNavigationDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let number = document\n        .session\n        .session()\n        .document()\n        .pages\n        .iter()\n        .find(|page| page.id == request.page_id)\n        .ok_or_else(|| CommandError::new(\"page_missing\", \"The requested page no longer exists.\"))?\n        .layers\n        .len()\n        + 1;\n    let layer = empty_layer(format!(\"Layer {number}\"));\n    let layer_id = layer.id;\n    document\n        .session\n        .create_page_layer(request.page_id, layer)\n        .map_err(|error| CommandError::new(\"layer_create_failed\", error.to_string()))?;\n    document\n        .session\n        .set_active_page_layer(request.page_id, layer_id)\n        .map_err(|error| CommandError::new(\"activate_layer_failed\", error.to_string()))?;\n    document.session.clear_selection();\n    Ok(document_navigation_dto(&document))\n}\n\n#[tauri::command]\nfn delete_layer(\n    request: LayerIdRequest,\n    state: State<'_, DesktopState>,\n) -> Result<DocumentNavigationDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let layer_count = document\n        .session\n        .session()\n        .document()\n        .pages\n        .iter()\n        .find(|page| page.id == request.page_id)\n        .ok_or_else(|| CommandError::new(\"page_missing\", \"The requested page no longer exists.\"))?\n        .layers\n        .len();\n    if layer_count <= 1 {\n        return Err(CommandError::new(\n            \"last_layer_delete_blocked\",\n            \"A page must keep at least one local layer.\",\n        ));\n    }\n    document\n        .session\n        .delete_page_layer(request.page_id, request.layer_id)\n        .map_err(|error| CommandError::new(\"layer_delete_failed\", error.to_string()))?;\n    document.session.clear_selection();\n    Ok(document_navigation_dto(&document))\n}\n\n#[tauri::command]\nfn update_layer_properties(\n    request: UpdateLayerPropertiesRequest,\n    state: State<'_, DesktopState>,\n) -> Result<DocumentNavigationDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let name = structure_name(&request.name)?;\n    let draw_color = document\n        .session\n        .session()\n        .document()\n        .pages\n        .iter()\n        .find(|page| page.id == request.page_id)\n        .and_then(|page| page.layers.iter().find(|layer| layer.id == request.layer_id))\n        .ok_or_else(|| CommandError::new(\"layer_missing\", \"The requested layer no longer exists.\"))?\n        .draw_color;\n    document\n        .session\n        .set_page_layer_properties(\n            request.page_id,\n            request.layer_id,\n            name,\n            request.visible,\n            request.locked,\n            draw_color,\n        )\n        .map_err(|error| CommandError::new(\"layer_properties_failed\", error.to_string()))?;\n    if !request.visible {\n        document.session.clear_selection();\n    }\n    Ok(document_navigation_dto(&document))\n}\n\n#[tauri::command]\nasync fn open_renderer_benchmark""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """fn element_edit_result_dto(document: &DesktopDocument) -> ElementEditResultDto {\n""",
    """fn document_navigation_dto(document: &DesktopDocument) -> DocumentNavigationDto {\n    let session = document.session.session();\n    let pages = session\n        .document()\n        .pages\n        .iter()\n        .map(|page| PageNavigationDto {\n            page_id: page.id,\n            name: page.name.clone(),\n            size_mm: page.size_mm,\n            layers: page\n                .layers\n                .iter()\n                .map(|layer| LayerNavigationDto {\n                    layer_id: layer.id,\n                    name: layer.name.clone(),\n                    visible: layer.visible,\n                    locked: layer.locked,\n                    element_count: layer.scene.elements.len(),\n                })\n                .collect(),\n        })\n        .collect();\n    DocumentNavigationDto {\n        state: document_state_dto(document),\n        pages,\n        active_page_id: session.active_page_id(),\n        active_layer_id: document.session.active_page_layer_id(),\n    }\n}\n\nfn structure_name(value: &str) -> Result<String, CommandError> {\n    let trimmed = value.trim();\n    if trimmed.is_empty() {\n        return Err(CommandError::new(\n            \"invalid_structure_name\",\n            \"Page and layer names must not be empty.\",\n        ));\n    }\n    Ok(trimmed.to_owned())\n}\n\nfn element_edit_result_dto(document: &DesktopDocument) -> ElementEditResultDto {\n""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """fn blank_document_artifact() -> NextArtifact {\n    let page_id = PageId::new();\n    let layer_id = LayerId::new();\n    NextArtifact::document(Document {\n        id: DocumentId::new(),\n        name: UNTITLED_DOCUMENT_NAME.to_owned(),\n        defaults: desktop_document_defaults(),\n        master_layers: Vec::new(),\n        pages: vec![Page {\n            id: page_id,\n            name: \"Page 1\".to_owned(),\n            size_mm: Size {\n                width: 210.0,\n                height: 297.0,\n            },\n            layers: vec![Layer {\n                id: layer_id,\n                name: \"Layer 1\".to_owned(),\n                visible: true,\n                locked: false,\n                draw_color: None,\n                scene: Scene {\n                    roots: Vec::new(),\n                    elements: Vec::new(),\n                },\n            }],\n        }],\n        styles: Vec::new(),\n        assets: Vec::new(),\n        import: None,\n    })\n}\n""",
    """fn empty_layer(name: String) -> Layer {\n    Layer {\n        id: LayerId::new(),\n        name,\n        visible: true,\n        locked: false,\n        draw_color: None,\n        scene: Scene::default(),\n    }\n}\n\nfn empty_page(name: String, size_mm: Size) -> Page {\n    Page {\n        id: PageId::new(),\n        name,\n        size_mm,\n        layers: vec![empty_layer(\"Layer 1\".to_owned())],\n    }\n}\n\nfn blank_document_artifact() -> NextArtifact {\n    NextArtifact::document(Document {\n        id: DocumentId::new(),\n        name: UNTITLED_DOCUMENT_NAME.to_owned(),\n        defaults: desktop_document_defaults(),\n        master_layers: Vec::new(),\n        pages: vec![empty_page(\n            \"Page 1\".to_owned(),\n            Size {\n                width: 210.0,\n                height: 297.0,\n            },\n        )],\n        styles: Vec::new(),\n        assets: Vec::new(),\n        import: None,\n    })\n}\n""",
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    """            document_state,\n            candidate_page_presentation,\n""",
    """            document_state,\n            document_navigation,\n            activate_page,\n            activate_layer,\n            create_page,\n            delete_page,\n            update_page_properties,\n            create_layer,\n            delete_layer,\n            update_layer_properties,\n            candidate_page_presentation,\n""",
)

replace_once(
    "apps/desktop/src-tauri/build.rs",
    """            \"document_state\",\n            \"candidate_page_presentation\",\n""",
    """            \"document_state\",\n            \"document_navigation\",\n            \"activate_page\",\n            \"activate_layer\",\n            \"create_page\",\n            \"delete_page\",\n            \"update_page_properties\",\n            \"create_layer\",\n            \"delete_layer\",\n            \"update_layer_properties\",\n            \"candidate_page_presentation\",\n""",
)


# --- HTML structure panel ---
replace_once(
    "apps/desktop/ui/index.html",
    """            <span class=\"phase-label\">Phase 1 desktop shell</span>\n""",
    """            <span class=\"phase-label\">Desktop editor preview</span>\n""",
)

replace_once(
    "apps/desktop/ui/index.html",
    """          <p id=\"renderer-stats\" class=\"renderer-stats\">Candidate renderer awaiting desktop runtime.</p>\n\n          <section class=\"selection-inspector\"""",
    """          <p id=\"renderer-stats\" class=\"renderer-stats\">Candidate renderer awaiting desktop runtime.</p>\n\n          <section class=\"document-structure\" aria-labelledby=\"document-structure-title\">\n            <div class=\"document-structure-heading\">\n              <h2 id=\"document-structure-title\">Pages &amp; layers</h2>\n            </div>\n\n            <label class=\"property-field\">\n              Page\n              <select id=\"page-select\" aria-label=\"Active page\"></select>\n            </label>\n            <div class=\"structure-actions\">\n              <button id=\"add-page\" type=\"button\">New page</button>\n              <button id=\"delete-page\" type=\"button\">Delete page</button>\n            </div>\n            <form id=\"page-properties-form\" class=\"structure-form\" hidden>\n              <label class=\"property-field\">Name <input id=\"page-name\" required /></label>\n              <div class=\"property-grid\">\n                <label>Width <input id=\"page-width\" type=\"number\" min=\"0.1\" step=\"0.1\" required /></label>\n                <label>Height <input id=\"page-height\" type=\"number\" min=\"0.1\" step=\"0.1\" required /></label>\n              </div>\n              <button id=\"apply-page-properties\" type=\"submit\">Apply page</button>\n            </form>\n\n            <label class=\"property-field structure-layer-field\">\n              Layer\n              <select id=\"layer-select\" aria-label=\"Active layer\"></select>\n            </label>\n            <div class=\"structure-actions\">\n              <button id=\"add-layer\" type=\"button\">New layer</button>\n              <button id=\"delete-layer\" type=\"button\">Delete layer</button>\n            </div>\n            <form id=\"layer-properties-form\" class=\"structure-form\" hidden>\n              <label class=\"property-field\">Name <input id=\"layer-name\" required /></label>\n              <div class=\"structure-checks\">\n                <label><input id=\"layer-visible\" type=\"checkbox\" /> Visible</label>\n                <label><input id=\"layer-locked\" type=\"checkbox\" /> Locked</label>\n              </div>\n              <p id=\"layer-element-count\" class=\"property-note\"></p>\n              <button id=\"apply-layer-properties\" type=\"submit\">Apply layer</button>\n            </form>\n          </section>\n\n          <section class=\"selection-inspector\"""",
)


# --- JS state/render/actions ---
replace_once(
    "apps/desktop/ui/app.js",
    """  rendererStats: document.querySelector('#renderer-stats'),\n  selectionSummary: document.querySelector('#selection-summary'),\n""",
    """  rendererStats: document.querySelector('#renderer-stats'),\n  pageSelect: document.querySelector('#page-select'),\n  addPage: document.querySelector('#add-page'),\n  deletePage: document.querySelector('#delete-page'),\n  pagePropertiesForm: document.querySelector('#page-properties-form'),\n  pageName: document.querySelector('#page-name'),\n  pageWidth: document.querySelector('#page-width'),\n  pageHeight: document.querySelector('#page-height'),\n  applyPageProperties: document.querySelector('#apply-page-properties'),\n  layerSelect: document.querySelector('#layer-select'),\n  addLayer: document.querySelector('#add-layer'),\n  deleteLayer: document.querySelector('#delete-layer'),\n  layerPropertiesForm: document.querySelector('#layer-properties-form'),\n  layerName: document.querySelector('#layer-name'),\n  layerVisible: document.querySelector('#layer-visible'),\n  layerLocked: document.querySelector('#layer-locked'),\n  layerElementCount: document.querySelector('#layer-element-count'),\n  applyLayerProperties: document.querySelector('#apply-layer-properties'),\n  selectionSummary: document.querySelector('#selection-summary'),\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """  elements.deleteSelection,\n  elements.applyProperties,\n  elements.rendererBenchmark,\n""",
    """  elements.deleteSelection,\n  elements.applyProperties,\n  elements.addPage,\n  elements.deletePage,\n  elements.applyPageProperties,\n  elements.addLayer,\n  elements.deleteLayer,\n  elements.applyLayerProperties,\n  elements.rendererBenchmark,\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """let currentPresentation = null;\nlet currentSelectionProperties = null;\nlet keyboardSurface = null;\n""",
    """let currentPresentation = null;\nlet currentSelectionProperties = null;\nlet currentNavigation = null;\nlet isBusy = false;\nlet keyboardSurface = null;\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """function setBusy(busy) {\n  for (const button of actionButtons) {\n    button.disabled = busy;\n  }\n  if (!busy) {\n    const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n    elements.deleteSelection.disabled = selectionCount === 0;\n    elements.applyProperties.disabled = !currentSelectionProperties?.primary;\n  }\n}\n""",
    """function setBusy(busy) {\n  isBusy = busy;\n  for (const button of actionButtons) {\n    button.disabled = busy;\n  }\n  elements.pageSelect.disabled = busy;\n  elements.layerSelect.disabled = busy;\n  if (!busy) {\n    const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n    elements.deleteSelection.disabled = selectionCount === 0;\n    elements.applyProperties.disabled = !currentSelectionProperties?.primary;\n    updateStructureDisabledState();\n  }\n}\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """function renderPresentationStats(presentation) {\n""",
    """function updateStructureDisabledState() {\n  const pages = currentNavigation?.pages ?? [];\n  const activePage = pages.find((page) => page.pageId === currentNavigation?.activePageId) ?? null;\n  elements.deletePage.disabled = isBusy || pages.length <= 1 || !activePage;\n  elements.addLayer.disabled = isBusy || !activePage;\n  elements.deleteLayer.disabled = isBusy || !activePage || activePage.layers.length <= 1;\n  elements.applyPageProperties.disabled = isBusy || !activePage;\n  const activeLayer =\n    activePage?.layers.find((layer) => layer.layerId === currentNavigation?.activeLayerId) ?? null;\n  elements.applyLayerProperties.disabled = isBusy || !activeLayer;\n}\n\nfunction renderNavigation(navigation) {\n  currentNavigation = navigation;\n  const pages = navigation?.pages ?? [];\n  const pageFragment = document.createDocumentFragment();\n  for (const page of pages) {\n    const option = document.createElement('option');\n    option.value = page.pageId;\n    option.textContent = page.name;\n    pageFragment.append(option);\n  }\n  elements.pageSelect.replaceChildren(pageFragment);\n  if (navigation?.activePageId) {\n    elements.pageSelect.value = navigation.activePageId;\n  }\n\n  const activePage = pages.find((page) => page.pageId === navigation?.activePageId) ?? null;\n  elements.pagePropertiesForm.hidden = !activePage;\n  if (activePage) {\n    elements.pageName.value = activePage.name;\n    elements.pageWidth.value = String(activePage.sizeMm.width);\n    elements.pageHeight.value = String(activePage.sizeMm.height);\n  }\n\n  const layerFragment = document.createDocumentFragment();\n  for (const layer of activePage?.layers ?? []) {\n    const option = document.createElement('option');\n    option.value = layer.layerId;\n    const flags = `${layer.visible ? '' : ' · hidden'}${layer.locked ? ' · locked' : ''}`;\n    option.textContent = `${layer.name}${flags}`;\n    layerFragment.append(option);\n  }\n  elements.layerSelect.replaceChildren(layerFragment);\n  if (navigation?.activeLayerId) {\n    elements.layerSelect.value = navigation.activeLayerId;\n  }\n\n  const activeLayer =\n    activePage?.layers.find((layer) => layer.layerId === navigation?.activeLayerId) ?? null;\n  elements.layerPropertiesForm.hidden = !activeLayer;\n  if (activeLayer) {\n    elements.layerName.value = activeLayer.name;\n    elements.layerVisible.checked = activeLayer.visible;\n    elements.layerLocked.checked = activeLayer.locked;\n    elements.layerElementCount.textContent = `${activeLayer.elementCount} stored element${activeLayer.elementCount === 1 ? '' : 's'}`;\n  } else {\n    elements.layerElementCount.textContent = '';\n  }\n  updateStructureDisabledState();\n}\n\nasync function refreshNavigation() {\n  if (!invoke) {\n    return null;\n  }\n  try {\n    const navigation = await invoke('document_navigation');\n    renderNavigation(navigation);\n    return navigation;\n  } catch (error) {\n    setStatus(formatInvokeError(error));\n    return null;\n  }\n}\n\nfunction renderPresentationStats(presentation) {\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """    renderPresentationStats(presentation);\n    renderRulers(presentation);\n    await refreshSelectionProperties();\n    return presentation;\n""",
    """    renderPresentationStats(presentation);\n    renderRulers(presentation);\n    await Promise.all([refreshSelectionProperties(), refreshNavigation()]);\n    return presentation;\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """async function createBasicElement(kind) {\n""",
    """function clearLocalSelection() {\n  svgSurface.setSelection([]);\n  keyboardSurface?.syncSelectionState([]);\n}\n\nasync function runStructureAction(\n  command,\n  args,\n  message,\n  { persistent = true, preserveSelection = false } = {},\n) {\n  if (!invoke) {\n    return;\n  }\n  setBusy(true);\n  try {\n    const result = await invoke(command, args);\n    if (result?.state) {\n      renderState(result.state);\n    }\n    renderNavigation(result);\n    if (!preserveSelection) {\n      clearLocalSelection();\n    }\n    await refreshPresentation({ preserveSelection });\n    if (persistent) {\n      scheduleRecoverySync(250);\n    }\n    setStatus(message);\n  } catch (error) {\n    setStatus(formatInvokeError(error));\n  } finally {\n    setBusy(false);\n  }\n}\n\nasync function createBasicElement(kind) {\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """    svgSurface.setSelection([]);\n    keyboardSurface?.syncSelectionState([]);\n""",
    """    clearLocalSelection();\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """elements.addRectangle.addEventListener('click', () => {\n""",
    """elements.pageSelect.addEventListener('change', () => {\n  const pageId = elements.pageSelect.value;\n  if (pageId) {\n    void runStructureAction(\n      'activate_page',\n      { request: { pageId } },\n      'Active page changed',\n      { persistent: false },\n    );\n  }\n});\n\nelements.layerSelect.addEventListener('change', () => {\n  const pageId = currentNavigation?.activePageId;\n  const layerId = elements.layerSelect.value;\n  if (pageId && layerId) {\n    void runStructureAction(\n      'activate_layer',\n      { request: { pageId, layerId } },\n      'Active layer changed',\n      { persistent: false },\n    );\n  }\n});\n\nelements.addPage.addEventListener('click', () => {\n  void runStructureAction('create_page', undefined, 'Page created');\n});\n\nelements.deletePage.addEventListener('click', () => {\n  const pageId = currentNavigation?.activePageId;\n  if (pageId) {\n    void runStructureAction('delete_page', { request: { pageId } }, 'Page deleted');\n  }\n});\n\nelements.pagePropertiesForm.addEventListener('submit', (event) => {\n  event.preventDefault();\n  const pageId = currentNavigation?.activePageId;\n  const width = Number(elements.pageWidth.value);\n  const height = Number(elements.pageHeight.value);\n  if (!pageId || !Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {\n    setStatus('Page width and height must be finite positive values');\n    return;\n  }\n  void runStructureAction(\n    'update_page_properties',\n    {\n      request: {\n        pageId,\n        name: elements.pageName.value,\n        sizeMm: { width, height },\n      },\n    },\n    'Page properties updated',\n    { preserveSelection: true },\n  );\n});\n\nelements.addLayer.addEventListener('click', () => {\n  const pageId = currentNavigation?.activePageId;\n  if (pageId) {\n    void runStructureAction('create_layer', { request: { pageId } }, 'Layer created');\n  }\n});\n\nelements.deleteLayer.addEventListener('click', () => {\n  const pageId = currentNavigation?.activePageId;\n  const layerId = currentNavigation?.activeLayerId;\n  if (pageId && layerId) {\n    void runStructureAction('delete_layer', { request: { pageId, layerId } }, 'Layer deleted');\n  }\n});\n\nelements.layerPropertiesForm.addEventListener('submit', (event) => {\n  event.preventDefault();\n  const pageId = currentNavigation?.activePageId;\n  const layerId = currentNavigation?.activeLayerId;\n  if (!pageId || !layerId) {\n    return;\n  }\n  void runStructureAction(\n    'update_layer_properties',\n    {\n      request: {\n        pageId,\n        layerId,\n        name: elements.layerName.value,\n        visible: elements.layerVisible.checked,\n        locked: elements.layerLocked.checked,\n      },\n    },\n    'Layer properties updated',\n    { preserveSelection: elements.layerVisible.checked },\n  );\n});\n\nelements.addRectangle.addEventListener('click', () => {\n""",
)

replace_once(
    "apps/desktop/ui/app.js",
    """svgSurface.setInteractionSettings(interactionSettings);\nrenderInteractionButtons();\n""",
    """svgSurface.setInteractionSettings(interactionSettings);\nrenderInteractionButtons();\nrenderNavigation(null);\n""",
)


# --- CSS for structure panel ---
append_text(
    "apps/desktop/ui/styles.css",
    r"""
.document-structure {
  margin-top: 16px;
  padding-top: 14px;
  border-top: 1px solid color-mix(in srgb, currentColor 18%, transparent);
}

.document-structure-heading,
.selection-inspector-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.document-structure h2 {
  margin: 0 0 10px;
  font-size: 12px;
  line-height: 1.2;
}

.document-structure select,
.document-structure input,
.selection-inspector input,
.selection-inspector textarea {
  width: 100%;
  min-width: 0;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface);
  color: var(--text);
  font: inherit;
  font-size: 12px;
}

.document-structure select,
.document-structure input,
.selection-inspector input {
  min-height: 32px;
  padding: 0 8px;
}

.document-structure select:focus-visible,
.document-structure input:focus-visible,
.selection-inspector input:focus-visible,
.selection-inspector textarea:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.structure-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 7px;
  margin-top: 7px;
}

.structure-actions button,
.structure-form button {
  width: 100%;
}

.structure-form {
  display: grid;
  gap: 8px;
  margin-top: 9px;
  padding: 9px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--surface-subtle);
}

.structure-layer-field {
  margin-top: 14px;
}

.structure-checks {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  color: var(--muted);
  font-size: 11px;
}

.structure-checks label {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.structure-checks input {
  width: auto;
  min-height: auto;
  margin: 0;
}
""",
)

print("Prepared page/layer management slice")
