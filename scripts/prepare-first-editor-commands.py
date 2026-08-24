from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if text.count(old) != 1:
        raise SystemExit(f"{label}: expected exactly one anchor, found {text.count(old)}")
    return text.replace(old, new, 1)


def write(path: str, content: str) -> None:
    Path(path).write_text(content, encoding="utf-8")


# ---------------------------------------------------------------------------
# app-core: expose typed constructive commands while keeping EditorSession
# mutable access private to the application layer.
# ---------------------------------------------------------------------------
app_path = Path("crates/app-core/src/lib.rs")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    "use editor_core::{EditCommand, EditorError, EditorSession, HistoryStateId};",
    "use editor_core::{\n    EditCommand, EditTransaction, EditorError, EditorSession, HistoryStateId, LayerTarget,\n};",
    "app-core editor imports",
)
app = replace_once(
    app,
    "use next_domain::{ElementId, NextArtifact, Point};",
    "use next_domain::{Element, ElementId, NextArtifact, Point, Rect, TextBlock};",
    "app-core domain imports",
)
old_move = '''    pub fn commit_move_elements(
        &mut self,
        element_ids: Vec<ElementId>,
        delta_mm: Point,
    ) -> Result<bool, ApplicationError> {
        let changed = self
            .runtime
            .session_mut()
            .execute(EditCommand::MoveElements {
                element_ids,
                delta_mm,
            })?;
        self.sync_editor_saved_marker();
        Ok(changed)
    }
'''
new_editing = '''    fn execute_edit(&mut self, command: EditCommand) -> Result<bool, ApplicationError> {
        let changed = self.runtime.session_mut().execute(command)?;
        self.sync_editor_saved_marker();
        Ok(changed)
    }

    fn execute_edit_transaction(
        &mut self,
        transaction: EditTransaction,
    ) -> Result<bool, ApplicationError> {
        let changed = self.runtime.session_mut().execute_transaction(transaction)?;
        self.sync_editor_saved_marker();
        Ok(changed)
    }

    pub fn commit_move_elements(
        &mut self,
        element_ids: Vec<ElementId>,
        delta_mm: Point,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::MoveElements {
            element_ids,
            delta_mm,
        })
    }

    /// Create one element through the editor-core semantic command boundary.
    pub fn create_element(
        &mut self,
        target: LayerTarget,
        element: Element,
        z_index: Option<usize>,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::CreateElement {
            target,
            element,
            z_index,
        })
    }

    /// Delete a selection as one semantic history step.
    pub fn delete_elements(
        &mut self,
        element_ids: Vec<ElementId>,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::DeleteElements { element_ids })
    }

    /// Commit bounds, rotation and an optional text replacement atomically.
    ///
    /// `text_update == None` leaves text untouched. `Some(None)` removes the text
    /// block, while `Some(Some(_))` replaces it.
    pub fn commit_element_properties(
        &mut self,
        element_id: ElementId,
        bounds_mm: Rect,
        rotation_deg: f64,
        text_update: Option<Option<TextBlock>>,
    ) -> Result<bool, ApplicationError> {
        let mut transaction = EditTransaction::default();
        transaction.push(EditCommand::SetBounds {
            element_id,
            bounds_mm,
        });
        transaction.push(EditCommand::SetRotation {
            element_id,
            rotation_deg,
        });
        if let Some(text) = text_update {
            transaction.push(EditCommand::SetText { element_id, text });
        }
        self.execute_edit_transaction(transaction)
    }
'''
app = replace_once(app, old_move, new_editing, "app-core move method")

insert_test = r'''

    #[test]
    fn constructive_application_commands_share_editor_history_and_dirty_state() {
        let (artifact, _) = fixture();
        let mut app = ApplicationSession::from_artifact(artifact).unwrap();
        let target = app.session().active_layer().unwrap();
        let created_id = ElementId::new();
        let created = Element {
            id: created_id,
            name: "Created rectangle".to_owned(),
            bounds_mm: Rect {
                x: 15.0,
                y: 25.0,
                width: 40.0,
                height: 20.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
            import: None,
        };

        let initial = app.session().current_history_state();
        assert!(app.create_element(target, created, None).unwrap());
        let after_create = app.session().current_history_state();
        assert_ne!(after_create, initial);
        assert!(app.is_dirty());
        assert!(app.session().document().pages[0].layers[0]
            .scene
            .elements
            .iter()
            .any(|element| element.id == created_id));

        let updated_bounds = Rect {
            x: 30.0,
            y: 35.0,
            width: 55.0,
            height: 28.0,
        };
        assert!(app
            .commit_element_properties(created_id, updated_bounds, 22.5, None)
            .unwrap());
        let created = app.session().document().pages[0].layers[0]
            .scene
            .elements
            .iter()
            .find(|element| element.id == created_id)
            .unwrap();
        assert_eq!(created.bounds_mm, updated_bounds);
        assert_eq!(created.rotation_deg, 22.5);

        assert!(app.delete_elements(vec![created_id]).unwrap());
        assert!(!app.session().document().pages[0].layers[0]
            .scene
            .elements
            .iter()
            .any(|element| element.id == created_id));
        assert!(app.undo().unwrap());
        assert!(app.session().document().pages[0].layers[0]
            .scene
            .elements
            .iter()
            .any(|element| element.id == created_id));
        assert!(app.undo().unwrap());
        assert!(app.undo().unwrap());
        assert_eq!(app.session().current_history_state(), initial);
        assert!(!app.is_dirty());
    }
'''
last_close = app.rfind("\n}")
if last_close < 0:
    raise SystemExit("app-core tests: module close not found")
app = app[:last_close] + insert_test + app[last_close:]
write(str(app_path), app)


# ---------------------------------------------------------------------------
# Tauri desktop boundary: create/delete/property commands and selection DTO.
# ---------------------------------------------------------------------------
desktop_path = Path("apps/desktop/src-tauri/src/lib.rs")
desktop = desktop_path.read_text(encoding="utf-8")
desktop = replace_once(
    desktop,
    "use editor_runtime::RecoveryPlan;",
    "use editor_core::LayerTarget;\nuse editor_runtime::RecoveryPlan;",
    "desktop editor-core import",
)
desktop = replace_once(
    desktop,
    '''use next_domain::{
    ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, ElementId, Layer, LayerId,
    NextArtifact, Page, PageId, Point, Rect, Scene, Size,
};''',
    '''use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Point, Rect, RichTextDocument,
    RichTextToken, Scene, Size, TextBlock, TextHorizontalAlignment, TextLayout, TextStyle,
    TextVerticalAlignment,
};''',
    "desktop next-domain imports",
)
request_anchor = '''#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveElementsRequest {
    element_ids: Vec<ElementId>,
    delta_mm: Point,
}
'''
request_block = request_anchor + '''
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BasicElementKind {
    Rectangle,
    Text,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBasicElementRequest {
    kind: BasicElementKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateElementPropertiesRequest {
    element_id: ElementId,
    bounds_mm: Rect,
    rotation_deg: f64,
    text: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ElementEditResultDto {
    state: DocumentStateDto,
    selected_element_ids: Vec<ElementId>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectionPropertiesDto {
    count: usize,
    primary: Option<ElementPropertiesDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ElementPropertiesDto {
    element_id: ElementId,
    name: String,
    element_type: &'static str,
    bounds_mm: Rect,
    rotation_deg: f64,
    text: Option<String>,
    text_editable: bool,
}
'''
desktop = replace_once(desktop, request_anchor, request_block, "desktop request DTOs")

selection_anchor = '''#[tauri::command]
fn new_document(state: State<'_, DesktopState>) -> Result<DocumentActionDto, CommandError> {
'''
commands = r'''#[tauri::command]
fn selection_properties(
    state: State<'_, DesktopState>,
) -> Result<SelectionPropertiesDto, CommandError> {
    let document = lock_document(&state)?;
    let session = document.session.session();
    let selected: Vec<_> = session.selection().iter().copied().collect();
    let primary = if selected.len() == 1 {
        let element = find_element(session.document(), selected[0]).ok_or_else(|| {
            CommandError::new(
                "selection_element_missing",
                "The selected element no longer exists in the current document.",
            )
        })?;
        Some(element_properties_dto(element))
    } else {
        None
    };
    Ok(SelectionPropertiesDto {
        count: selected.len(),
        primary,
    })
}

#[tauri::command]
fn create_basic_element(
    request: CreateBasicElementRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let (target, page_size) = {
        let session = document.session.session();
        let target = session.active_layer().ok_or_else(|| {
            CommandError::new(
                "no_active_layer",
                "The current document has no active layer for element creation.",
            )
        })?;
        let page_size = session
            .active_page_id()
            .and_then(|page_id| {
                session
                    .document()
                    .pages
                    .iter()
                    .find(|page| page.id == page_id)
                    .map(|page| page.size_mm)
            })
            .unwrap_or(Size {
                width: 210.0,
                height: 297.0,
            });
        (target, page_size)
    };

    let element_id = ElementId::new();
    let (name, width, height, kind, text) = match request.kind {
        BasicElementKind::Rectangle => (
            "Rectangle".to_owned(),
            40.0,
            25.0,
            ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
            None,
        ),
        BasicElementKind::Text => (
            "Text".to_owned(),
            60.0,
            20.0,
            ElementKind::Text,
            Some(simple_text_block("Text", TextStyle::default(), None)),
        ),
    };
    let bounds_mm = Rect {
        x: ((page_size.width - width) / 2.0).max(0.0),
        y: ((page_size.height - height) / 2.0).max(0.0),
        width,
        height,
    };
    let element = Element {
        id: element_id,
        name,
        bounds_mm,
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text,
        kind,
        import: None,
    };

    document
        .session
        .create_element(target, element, None)
        .map_err(|error| CommandError::new("element_create_failed", error.to_string()))?;
    document
        .session
        .set_selection([element_id])
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn delete_selection(
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
    if !selected.is_empty() {
        document
            .session
            .delete_elements(selected)
            .map_err(|error| CommandError::new("element_delete_failed", error.to_string()))?;
        document.session.clear_selection();
    }
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn update_element_properties(
    request: UpdateElementPropertiesRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let text_update = if let Some(text) = request.text {
        let existing = find_element(document.session.session().document(), request.element_id)
            .ok_or_else(|| {
                CommandError::new(
                    "element_properties_missing",
                    "The selected element no longer exists in the current document.",
                )
            })?;
        let Some(existing_text) = existing.text.as_ref() else {
            return Err(CommandError::new(
                "element_text_not_editable",
                "This element does not contain editable text.",
            ));
        };
        let (_, editable, common_style) = text_preview(existing_text);
        if !editable {
            return Err(CommandError::new(
                "element_text_not_editable",
                "This rich-text element cannot be flattened safely by the basic text editor.",
            ));
        }
        Some(Some(simple_text_block(
            &text,
            common_style.unwrap_or_default(),
            Some(existing_text.layout),
        )))
    } else {
        None
    };

    document
        .session
        .commit_element_properties(
            request.element_id,
            request.bounds_mm,
            request.rotation_deg,
            text_update,
        )
        .map_err(|error| CommandError::new("element_properties_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

'''
desktop = replace_once(desktop, selection_anchor, commands + selection_anchor, "desktop edit commands")

helper_anchor = '''fn lock_document<'a>(
'''
helpers = r'''fn element_edit_result_dto(document: &DesktopDocument) -> ElementEditResultDto {
    ElementEditResultDto {
        state: document_state_dto(document),
        selected_element_ids: document
            .session
            .session()
            .selection()
            .iter()
            .copied()
            .collect(),
    }
}

fn find_element(document: &Document, element_id: ElementId) -> Option<&Element> {
    document
        .master_layers
        .iter()
        .chain(document.pages.iter().flat_map(|page| page.layers.iter()))
        .find_map(|layer| {
            layer
                .scene
                .elements
                .iter()
                .find(|element| element.id == element_id)
        })
}

fn element_properties_dto(element: &Element) -> ElementPropertiesDto {
    let (text, text_editable) = match element.text.as_ref() {
        Some(block) => {
            let (preview, editable, _) = text_preview(block);
            (Some(preview), editable)
        }
        None => (None, false),
    };
    ElementPropertiesDto {
        element_id: element.id,
        name: element.name.clone(),
        element_type: element_type_name(&element.kind),
        bounds_mm: element.bounds_mm,
        rotation_deg: element.rotation_deg,
        text,
        text_editable,
    }
}

fn element_type_name(kind: &ElementKind) -> &'static str {
    match kind {
        ElementKind::Text => "Text",
        ElementKind::Rectangle { .. } => "Rectangle",
        ElementKind::Ellipse => "Ellipse",
        ElementKind::StraightConnector { .. } => "Straight connector",
        ElementKind::OrthogonalConnector { .. } => "Orthogonal connector",
        ElementKind::Image { .. } => "Image",
        ElementKind::Metafile { .. } => "Metafile",
        ElementKind::Group { .. } => "Group",
        ElementKind::Polygon { .. } => "Polygon",
        ElementKind::Flowchart { .. } => "Flowchart",
        ElementKind::Curve { .. } => "Curve",
        ElementKind::LayerReference { .. } => "Layer reference",
    }
}

fn text_preview(block: &TextBlock) -> (String, bool, Option<TextStyle>) {
    let mut preview = String::new();
    let mut common_style: Option<TextStyle> = None;
    let mut editable = block.content.tail.is_none() && block.content.diagnostics.is_empty();
    for token in &block.content.tokens {
        match token {
            RichTextToken::Text { text, style } => {
                preview.push_str(text);
                if let Some(existing) = common_style.as_ref() {
                    if existing != style {
                        editable = false;
                    }
                } else {
                    common_style = Some(style.clone());
                }
            }
            RichTextToken::NewLine => preview.push('\n'),
            RichTextToken::PageNumber { .. } => {
                preview.push_str("{page}");
                editable = false;
            }
            RichTextToken::PageCount { .. } => {
                preview.push_str("{pages}");
                editable = false;
            }
            RichTextToken::PageName { .. } => {
                preview.push_str("{page name}");
                editable = false;
            }
            RichTextToken::SymbolGlyph { legacy_glyph, .. } => {
                preview.push(*legacy_glyph);
                editable = false;
            }
        }
    }
    (preview, editable, common_style)
}

fn simple_text_block(
    text: &str,
    style: TextStyle,
    layout: Option<TextLayout>,
) -> TextBlock {
    let mut tokens = Vec::new();
    let mut lines = text.split('\n').peekable();
    while let Some(line) = lines.next() {
        tokens.push(RichTextToken::Text {
            text: line.to_owned(),
            style: style.clone(),
        });
        if lines.peek().is_some() {
            tokens.push(RichTextToken::NewLine);
        }
    }
    TextBlock {
        content: RichTextDocument {
            tokens,
            tail: None,
            diagnostics: Vec::new(),
        },
        layout: layout.unwrap_or(TextLayout {
            horizontal: TextHorizontalAlignment::Left,
            vertical: TextVerticalAlignment::Top,
            margin_mm: 1.0,
        }),
    }
}

'''
desktop = replace_once(desktop, helper_anchor, helpers + helper_anchor, "desktop helpers")

handler_anchor = '''            candidate_page_presentation,
            set_selection,
            new_document,'''
handler_new = '''            candidate_page_presentation,
            set_selection,
            selection_properties,
            create_basic_element,
            delete_selection,
            update_element_properties,
            new_document,'''
desktop = replace_once(desktop, handler_anchor, handler_new, "desktop invoke handler")
write(str(desktop_path), desktop)


# ---------------------------------------------------------------------------
# Tauri command manifest build list.
# ---------------------------------------------------------------------------
build_path = Path("apps/desktop/src-tauri/build.rs")
build = build_path.read_text(encoding="utf-8")
build = replace_once(
    build,
    '''            "candidate_page_presentation",
            "set_selection",
            "new_document",''',
    '''            "candidate_page_presentation",
            "set_selection",
            "selection_properties",
            "create_basic_element",
            "delete_selection",
            "update_element_properties",
            "new_document",''',
    "tauri build command manifest",
)
write(str(build_path), build)


# ---------------------------------------------------------------------------
# Desktop HTML: constructive toolbar and single-selection inspector.
# ---------------------------------------------------------------------------
html_path = Path("apps/desktop/ui/index.html")
html = html_path.read_text(encoding="utf-8")
html = replace_once(
    html,
    '''          <button id="toggle-grid" class="toggle-button" type="button" aria-pressed="true">Grid</button>
          <button id="toggle-snap" class="toggle-button" type="button" aria-pressed="true">Snap</button>
          <span class="toolbar-separator" aria-hidden="true"></span>
          <button id="renderer-benchmark" type="button" title="Open the isolated ADR-019 native renderer benchmark">4K benchmark</button>''',
    '''          <button id="toggle-grid" class="toggle-button" type="button" aria-pressed="true">Grid</button>
          <button id="toggle-snap" class="toggle-button" type="button" aria-pressed="true">Snap</button>
          <span class="toolbar-separator" aria-hidden="true"></span>
          <button id="add-rectangle" type="button" title="Create a rectangle on the active layer">Rectangle</button>
          <button id="add-text" type="button" title="Create a text box on the active layer">Text</button>
          <button id="delete-selection" type="button" title="Delete the current selection">Delete</button>
          <span class="toolbar-separator" aria-hidden="true"></span>
          <button id="renderer-benchmark" type="button" title="Open the isolated ADR-019 native renderer benchmark">4K benchmark</button>''',
    "desktop constructive toolbar",
)
html = replace_once(
    html,
    '''          <p id="renderer-stats" class="renderer-stats">Candidate renderer awaiting desktop runtime.</p>

          <p class="boundary-note">''',
    '''          <p id="renderer-stats" class="renderer-stats">Candidate renderer awaiting desktop runtime.</p>

          <section class="selection-inspector" aria-labelledby="selection-inspector-title">
            <div class="selection-inspector-heading">
              <h2 id="selection-inspector-title">Selection</h2>
              <span id="selection-summary">No selection</span>
            </div>
            <form id="selection-properties-form" hidden>
              <div class="selection-identity">
                <strong id="selection-name">—</strong>
                <span id="selection-type">—</span>
              </div>
              <div class="property-grid">
                <label>X <input id="property-x" type="number" step="0.1" /></label>
                <label>Y <input id="property-y" type="number" step="0.1" /></label>
                <label>Width <input id="property-width" type="number" min="0.1" step="0.1" /></label>
                <label>Height <input id="property-height" type="number" min="0.1" step="0.1" /></label>
              </div>
              <label class="property-field">Rotation° <input id="property-rotation" type="number" step="0.1" /></label>
              <label id="property-text-field" class="property-field" hidden>
                Text
                <textarea id="property-text" rows="4"></textarea>
              </label>
              <p id="property-text-note" class="property-note" hidden></p>
              <button id="apply-properties" class="primary" type="submit">Apply properties</button>
            </form>
          </section>

          <p class="boundary-note">''',
    "desktop selection inspector",
)
write(str(html_path), html)


# ---------------------------------------------------------------------------
# Frontend behavior.
# ---------------------------------------------------------------------------
js_path = Path("apps/desktop/ui/app.js")
js = js_path.read_text(encoding="utf-8")
js = replace_once(
    js,
    '''  toggleGrid: document.querySelector('#toggle-grid'),
  toggleSnap: document.querySelector('#toggle-snap'),
  rendererBenchmark: document.querySelector('#renderer-benchmark'),''',
    '''  toggleGrid: document.querySelector('#toggle-grid'),
  toggleSnap: document.querySelector('#toggle-snap'),
  addRectangle: document.querySelector('#add-rectangle'),
  addText: document.querySelector('#add-text'),
  deleteSelection: document.querySelector('#delete-selection'),
  rendererBenchmark: document.querySelector('#renderer-benchmark'),''',
    "frontend toolbar refs",
)
js = replace_once(
    js,
    '''  rendererStats: document.querySelector('#renderer-stats'),
  rulerX: document.querySelector('#ruler-x'),''',
    '''  rendererStats: document.querySelector('#renderer-stats'),
  selectionSummary: document.querySelector('#selection-summary'),
  selectionPropertiesForm: document.querySelector('#selection-properties-form'),
  selectionName: document.querySelector('#selection-name'),
  selectionType: document.querySelector('#selection-type'),
  propertyX: document.querySelector('#property-x'),
  propertyY: document.querySelector('#property-y'),
  propertyWidth: document.querySelector('#property-width'),
  propertyHeight: document.querySelector('#property-height'),
  propertyRotation: document.querySelector('#property-rotation'),
  propertyTextField: document.querySelector('#property-text-field'),
  propertyText: document.querySelector('#property-text'),
  propertyTextNote: document.querySelector('#property-text-note'),
  applyProperties: document.querySelector('#apply-properties'),
  rulerX: document.querySelector('#ruler-x'),''',
    "frontend inspector refs",
)
js = replace_once(
    js,
    '''  elements.undo,
  elements.redo,
  elements.rendererBenchmark,
];''',
    '''  elements.undo,
  elements.redo,
  elements.addRectangle,
  elements.addText,
  elements.deleteSelection,
  elements.applyProperties,
  elements.rendererBenchmark,
];''',
    "frontend action buttons",
)
js = replace_once(
    js,
    '''let currentPresentation = null;
let keyboardSurface = null;''',
    '''let currentPresentation = null;
let currentSelectionProperties = null;
let keyboardSurface = null;''',
    "frontend selection state",
)
js = replace_once(
    js,
    '''  onSelectionChange: (elementIds) => {
    keyboardSurface?.syncSelectionState(elementIds);
    syncSelection(elementIds);
  },''',
    '''  onSelectionChange: (elementIds) => {
    keyboardSurface?.syncSelectionState(elementIds);
    void syncSelection(elementIds);
  },''',
    "frontend selection callback",
)
old_sync = '''function syncSelection(elementIds) {
  if (!invoke) {
    return;
  }
  void invoke('set_selection', {
    request: { elementIds: [...elementIds] },
  }).catch((error) => {
    setStatus(formatInvokeError(error));
  });
}
'''
new_sync = r'''function renderSelectionProperties(details) {
  currentSelectionProperties = details;
  const count = Number(details?.count ?? 0);
  elements.selectionSummary.textContent =
    count === 0 ? 'No selection' : count === 1 ? '1 element' : `${count} elements`;
  elements.deleteSelection.disabled = count === 0;

  const primary = details?.primary ?? null;
  if (!primary) {
    elements.selectionPropertiesForm.hidden = true;
    return;
  }

  elements.selectionPropertiesForm.hidden = false;
  elements.selectionName.textContent = primary.name;
  elements.selectionType.textContent = primary.elementType;
  elements.propertyX.value = String(primary.boundsMm.x);
  elements.propertyY.value = String(primary.boundsMm.y);
  elements.propertyWidth.value = String(primary.boundsMm.width);
  elements.propertyHeight.value = String(primary.boundsMm.height);
  elements.propertyRotation.value = String(primary.rotationDeg);

  const hasText = primary.text !== null && primary.text !== undefined;
  elements.propertyTextField.hidden = !hasText;
  elements.propertyTextNote.hidden = !hasText || primary.textEditable;
  if (hasText) {
    elements.propertyText.value = primary.text;
    elements.propertyText.disabled = !primary.textEditable;
    if (!primary.textEditable) {
      elements.propertyTextNote.textContent =
        'Rich text is shown for reference; this basic editor will not flatten mixed formatting or dynamic fields.';
    }
  }
}

async function refreshSelectionProperties() {
  if (!invoke) {
    return null;
  }
  try {
    const details = await invoke('selection_properties');
    renderSelectionProperties(details);
    return details;
  } catch (error) {
    setStatus(formatInvokeError(error));
    return null;
  }
}

async function syncSelection(elementIds) {
  if (!invoke) {
    return;
  }
  try {
    await invoke('set_selection', {
      request: { elementIds: [...elementIds] },
    });
    await refreshSelectionProperties();
  } catch (error) {
    setStatus(formatInvokeError(error));
  }
}

async function createBasicElement(kind) {
  if (!invoke) {
    return;
  }
  setBusy(true);
  try {
    const result = await invoke('create_basic_element', { request: { kind } });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: false });
    svgSurface.setSelection(result.selectedElementIds ?? []);
    keyboardSurface?.syncSelectionState(result.selectedElementIds ?? []);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus(kind === 'text' ? 'Text box created' : 'Rectangle created');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function deleteCurrentSelection() {
  if (!invoke) {
    return;
  }
  setBusy(true);
  try {
    const result = await invoke('delete_selection');
    renderState(result.state);
    svgSurface.setSelection([]);
    keyboardSurface?.syncSelectionState([]);
    await refreshPresentation({ preserveSelection: false });
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus('Selection deleted');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

async function applyElementProperties(event) {
  event.preventDefault();
  const primary = currentSelectionProperties?.primary;
  if (!invoke || !primary) {
    return;
  }
  const numbers = {
    x: Number(elements.propertyX.value),
    y: Number(elements.propertyY.value),
    width: Number(elements.propertyWidth.value),
    height: Number(elements.propertyHeight.value),
    rotation: Number(elements.propertyRotation.value),
  };
  if (
    !Object.values(numbers).every(Number.isFinite) ||
    numbers.width <= 0 ||
    numbers.height <= 0
  ) {
    setStatus('Bounds must be finite and width/height must be greater than zero');
    return;
  }

  const request = {
    elementId: primary.elementId,
    boundsMm: {
      x: numbers.x,
      y: numbers.y,
      width: numbers.width,
      height: numbers.height,
    },
    rotationDeg: numbers.rotation,
  };
  if (primary.textEditable) {
    request.text = elements.propertyText.value;
  }

  setBusy(true);
  try {
    const result = await invoke('update_element_properties', { request });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: true });
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus('Element properties updated');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}
'''
js = replace_once(js, old_sync, new_sync, "frontend selection/edit functions")
js = replace_once(
    js,
    '''    renderPresentationStats(presentation);
    renderRulers(presentation);
    return presentation;''',
    '''    renderPresentationStats(presentation);
    renderRulers(presentation);
    await refreshSelectionProperties();
    return presentation;''',
    "frontend presentation selection refresh",
)
listener_anchor = '''elements.saveDocument.addEventListener('click', () => {
'''
listeners = r'''elements.addRectangle.addEventListener('click', () => {
  void createBasicElement('rectangle');
});

elements.addText.addEventListener('click', () => {
  void createBasicElement('text');
});

elements.deleteSelection.addEventListener('click', () => {
  void deleteCurrentSelection();
});

elements.selectionPropertiesForm.addEventListener('submit', (event) => {
  void applyElementProperties(event);
});

'''
js = replace_once(js, listener_anchor, listeners + listener_anchor, "frontend editing listeners")
write(str(js_path), js)


# ---------------------------------------------------------------------------
# Styling: append compact inspector styles without disturbing the established
# desktop shell layout.
# ---------------------------------------------------------------------------
css_path = Path("apps/desktop/ui/styles.css")
css = css_path.read_text(encoding="utf-8")
css += r'''

.selection-inspector {
  margin-top: 16px;
  padding-top: 14px;
  border-top: 1px solid color-mix(in srgb, currentColor 18%, transparent);
}

.selection-inspector-heading,
.selection-identity {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
}

.selection-inspector h2 {
  margin: 0;
  font-size: 0.95rem;
}

#selection-summary,
#selection-type,
.property-note {
  font-size: 0.78rem;
  opacity: 0.72;
}

#selection-properties-form {
  display: grid;
  gap: 10px;
  margin-top: 12px;
}

#selection-properties-form[hidden] {
  display: none;
}

.property-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.property-grid label,
.property-field {
  display: grid;
  gap: 4px;
  font-size: 0.78rem;
}

.property-grid input,
.property-field input,
.property-field textarea {
  width: 100%;
  box-sizing: border-box;
  font: inherit;
}

.property-field textarea {
  resize: vertical;
}

.property-note {
  margin: 0;
  line-height: 1.35;
}
'''
write(str(css_path), css)

print("Prepared first constructive desktop editor slice")
