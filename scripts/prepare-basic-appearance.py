from pathlib import Path


def replace_once(text, old, new, label):
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def patch(path, fn):
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    p.write_text(fn(text), encoding="utf-8")


# editor-core: semantic element-owned appearance style.
def patch_editor(text):
    text = replace_once(
        text,
        "    Artifact, Color, Connection, Connector, Document, Element, ElementId, ElementKind, Endpoint,\n    Layer, LayerId, NextArtifact, Page, PageId, Point, PortId, Rect, Scene, Size, StyleId,\n    TextBlock, ValidationReport,\n",
        "    Artifact, Color, Connection, Connector, Document, Element, ElementId, ElementKind,\n    ElementStyle, Endpoint, FillStyle, Layer, LayerId, NextArtifact, Page, PageId, Point, PortId,\n    Rect, Scene, Size, StrokeStyle, StyleId, TextBlock, ValidationReport,\n",
        "editor imports",
    )
    text = replace_once(
        text,
        "    SetElementStyle {\n        element_ids: Vec<ElementId>,\n        style_id: Option<StyleId>,\n    },\n    SetText {",
        "    SetElementStyle {\n        element_ids: Vec<ElementId>,\n        style_id: Option<StyleId>,\n    },\n    /// Replace the visual appearance of one element with an element-owned style.\n    /// The deterministic style ID prevents repeated edits from accumulating style records\n    /// and guarantees that imported/shared style records are never mutated in place.\n    SetElementAppearance {\n        element_id: ElementId,\n        stroke: Option<StrokeStyle>,\n        fill: Option<FillStyle>,\n        text_color: Option<Color>,\n    },\n    SetText {",
        "appearance command",
    )
    text = replace_once(
        text,
        "    #[error(\"style {0:?} does not exist\")]\n    StyleNotFound(StyleId),\n",
        "    #[error(\"style {0:?} does not exist\")]\n    StyleNotFound(StyleId),\n    #[error(\"appearance contains an invalid stroke width\")]\n    InvalidAppearance,\n    #[error(\"element-owned appearance style {0:?} collides with existing document state\")]\n    AppearanceStyleCollision(StyleId),\n",
        "appearance errors",
    )
    text = replace_once(
        text,
        "    SetElementStyles {\n        previous: Vec<(ElementId, Option<StyleId>)>,\n    },\n    SetText {",
        "    SetElementStyles {\n        previous: Vec<(ElementId, Option<StyleId>)>,\n    },\n    RestoreElementAppearance {\n        element_id: ElementId,\n        previous_style_id: Option<StyleId>,\n        dedicated_style_id: StyleId,\n        previous_dedicated_style: Option<ElementStyle>,\n    },\n    SetText {",
        "appearance undo",
    )
    text = replace_once(
        text,
        "        EditCommand::SetElementStyle {\n            element_ids,\n            style_id,\n        } => apply_set_element_style(document, element_ids, *style_id),\n        EditCommand::SetText { element_id, text } => apply_set_text(document, *element_id, text),",
        "        EditCommand::SetElementStyle {\n            element_ids,\n            style_id,\n        } => apply_set_element_style(document, element_ids, *style_id),\n        EditCommand::SetElementAppearance {\n            element_id,\n            stroke,\n            fill,\n            text_color,\n        } => apply_set_element_appearance(\n            document,\n            *element_id,\n            stroke,\n            fill,\n            *text_color,\n        ),\n        EditCommand::SetText { element_id, text } => apply_set_text(document, *element_id, text),",
        "appearance apply dispatch",
    )
    appearance_fn = r'''
fn apply_set_element_appearance(
    document: &mut Document,
    element_id: ElementId,
    stroke: &Option<StrokeStyle>,
    fill: &Option<FillStyle>,
    text_color: Option<Color>,
) -> Result<Option<AppliedCommand>, EditorError> {
    if stroke
        .as_ref()
        .is_some_and(|stroke| !stroke.width_mm.is_finite() || stroke.width_mm <= 0.0)
    {
        return Err(EditorError::InvalidAppearance);
    }
    ensure_element_editable(document, element_id)?;

    let dedicated_style_id = StyleId::v5(element_id.0, "diagramdesigner-next:element-appearance");
    let previous_style_id = find_element(document, element_id)
        .ok_or(EditorError::ElementNotFound(element_id))?
        .style_id;
    let previous_dedicated_style = document
        .styles
        .iter()
        .find(|style| style.id == dedicated_style_id)
        .cloned();

    let referenced_by_other = all_layers(document).any(|layer| {
        layer.scene.elements.iter().any(|element| {
            element.id != element_id && element.style_id == Some(dedicated_style_id)
        })
    });
    if referenced_by_other
        || (previous_dedicated_style.is_some() && previous_style_id != Some(dedicated_style_id))
    {
        return Err(EditorError::AppearanceStyleCollision(dedicated_style_id));
    }

    let next_style = ElementStyle {
        id: dedicated_style_id,
        stroke: stroke.clone(),
        fill: fill.clone(),
        text_color,
    };
    if previous_style_id == Some(dedicated_style_id)
        && previous_dedicated_style.as_ref() == Some(&next_style)
    {
        return Ok(None);
    }

    if let Some(existing) = document
        .styles
        .iter_mut()
        .find(|style| style.id == dedicated_style_id)
    {
        *existing = next_style;
    } else {
        document.styles.push(next_style);
    }
    find_element_mut(document, element_id)
        .ok_or(EditorError::HistoryInvariantViolation)?
        .style_id = Some(dedicated_style_id);

    Ok(Some(AppliedCommand {
        undo: UndoStep::RestoreElementAppearance {
            element_id,
            previous_style_id,
            dedicated_style_id,
            previous_dedicated_style,
        },
        // The command creates a style reference and therefore participates in domain validation.
        structural: true,
    }))
}

'''
    text = replace_once(
        text,
        "fn apply_set_text(\n",
        appearance_fn + "fn apply_set_text(\n",
        "appearance function",
    )
    undo_branch = r'''        UndoStep::RestoreElementAppearance {
            element_id,
            previous_style_id,
            dedicated_style_id,
            previous_dedicated_style,
        } => {
            find_element_mut(document, *element_id)
                .ok_or(EditorError::HistoryInvariantViolation)?
                .style_id = *previous_style_id;
            if let Some(previous) = previous_dedicated_style {
                if let Some(existing) = document
                    .styles
                    .iter_mut()
                    .find(|style| style.id == *dedicated_style_id)
                {
                    *existing = previous.clone();
                } else {
                    document.styles.push(previous.clone());
                }
            } else {
                document.styles.retain(|style| style.id != *dedicated_style_id);
            }
        }
'''
    text = replace_once(
        text,
        "        UndoStep::SetText { element_id, text } => {",
        undo_branch + "        UndoStep::SetText { element_id, text } => {",
        "appearance undo branch",
    )
    test = r'''
    #[test]
    fn appearance_edit_uses_element_owned_style_and_is_one_undoable_step() {
        let (base, first, second, _, _) = fixture(false);
        let shared = StyleId::new();
        let mut document = base.document().clone();
        document.styles.push(ElementStyle {
            id: shared,
            stroke: Some(StrokeStyle {
                width_mm: 0.4,
                color: Color::SystemPalette { index: 7 },
            }),
            fill: None,
            text_color: None,
        });
        find_element_mut(&mut document, first).unwrap().style_id = Some(shared);
        find_element_mut(&mut document, second).unwrap().style_id = Some(shared);
        let mut session = EditorSession::from_artifact(NextArtifact::document(document)).unwrap();
        let before = session.current_history_state();

        assert!(session
            .execute(EditCommand::SetElementAppearance {
                element_id: first,
                stroke: Some(StrokeStyle {
                    width_mm: 0.8,
                    color: Color::Rgba { r: 10, g: 20, b: 30, a: 255 },
                }),
                fill: Some(FillStyle {
                    color: Color::Rgba { r: 240, g: 230, b: 220, a: 255 },
                    gradient: None,
                }),
                text_color: None,
            })
            .unwrap());
        let after = session.current_history_state();
        assert_ne!(after, before);
        let dedicated = style_ref(&session, first).unwrap();
        assert_ne!(dedicated, shared);
        assert_eq!(style_ref(&session, second), Some(shared));
        assert_eq!(session.document().styles.len(), 2);
        assert_eq!(
            session.document().styles.iter().find(|style| style.id == shared).unwrap().stroke.as_ref().unwrap().color,
            Color::SystemPalette { index: 7 }
        );

        assert!(session.undo().unwrap());
        assert_eq!(session.current_history_state(), before);
        assert_eq!(style_ref(&session, first), Some(shared));
        assert!(session.document().styles.iter().all(|style| style.id != dedicated));

        assert!(session.redo().unwrap());
        assert_eq!(session.current_history_state(), after);
        assert_eq!(style_ref(&session, first), Some(dedicated));
        assert_eq!(session.document().styles.len(), 2);
    }

'''
    text = replace_once(
        text,
        "    #[test]\n    fn connector_endpoint_connection_is_canonical_and_undoable() {",
        test + "    #[test]\n    fn connector_endpoint_connection_is_canonical_and_undoable() {",
        "appearance editor test",
    )
    return text

patch("crates/editor-core/src/lib.rs", patch_editor)


# app-core: public appearance semantic boundary.
def patch_app_core(text):
    text = replace_once(
        text,
        "    Color, Connection, Element, ElementId, Layer, LayerId, NextArtifact, Page, PageId, Point,\n    PortId, Rect, Size, TextBlock,\n",
        "    Color, Connection, Element, ElementId, FillStyle, Layer, LayerId, NextArtifact, Page,\n    PageId, Point, PortId, Rect, Size, StrokeStyle, TextBlock,\n",
        "app core imports",
    )
    method = r'''
    /// Apply stroke/fill/text colour as one semantic history step. editor-core owns
    /// the deterministic per-element style identity and never mutates shared styles.
    pub fn set_element_appearance(
        &mut self,
        element_id: ElementId,
        stroke: Option<StrokeStyle>,
        fill: Option<FillStyle>,
        text_color: Option<Color>,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::SetElementAppearance {
            element_id,
            stroke,
            fill,
            text_color,
        })
    }

'''
    text = replace_once(
        text,
        "    /// Switch the active page without creating a persistent history step.\n",
        method + "    /// Switch the active page without creating a persistent history step.\n",
        "app appearance method",
    )
    test = r'''
    #[test]
    fn appearance_commit_is_one_application_history_step() {
        let (artifact, element_id) = fixture();
        let mut app = ApplicationSession::from_artifact(artifact).unwrap();
        let before = app.session().current_history_state();
        assert!(app
            .set_element_appearance(
                element_id,
                Some(StrokeStyle {
                    width_mm: 0.6,
                    color: Color::Rgba { r: 12, g: 34, b: 56, a: 255 },
                }),
                Some(FillStyle {
                    color: Color::Rgba { r: 200, g: 210, b: 220, a: 255 },
                    gradient: None,
                }),
                None,
            )
            .unwrap());
        let after = app.session().current_history_state();
        assert_ne!(after, before);
        assert!(app.is_dirty());
        assert!(app.undo().unwrap());
        assert_eq!(app.session().current_history_state(), before);
        assert!(!app.is_dirty());
        assert!(app.redo().unwrap());
        assert_eq!(app.session().current_history_state(), after);
    }

'''
    text = replace_once(
        text,
        "    #[test]\n    fn page_and_layer_commands_keep_navigation_transient_and_structure_in_history() {",
        test + "    #[test]\n    fn page_and_layer_commands_keep_navigation_transient_and_structure_in_history() {",
        "app appearance test",
    )
    return text

patch("crates/app-core/src/lib.rs", patch_app_core)


# Tauri command/DTO and ellipse creation.
def patch_tauri(text):
    text = replace_once(
        text,
        "    Element, ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle,\n    NextArtifact, NormalizedPoint, Page, PageId, Point, Port, PortId, Rect, RichTextDocument,\n    RichTextToken, Scene, Size, TextBlock, TextHorizontalAlignment, TextLayout, TextStyle,\n",
        "    Color, Element, ElementId, ElementKind, Endpoint, FillStyle, Layer, LayerId, LineStyle,\n    MarkerStyle, NextArtifact, NormalizedPoint, Page, PageId, Point, Port, PortId, Rect,\n    RichTextDocument, RichTextToken, Scene, Size, StrokeStyle, TextBlock, TextHorizontalAlignment,\n    TextLayout, TextStyle,\n",
        "tauri imports",
    )
    text = replace_once(
        text,
        "enum BasicElementKind {\n    Rectangle,\n    Text,\n}",
        "enum BasicElementKind {\n    Rectangle,\n    Ellipse,\n    Text,\n}",
        "ellipse enum",
    )
    dto = r'''
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateElementAppearanceRequest {
    element_id: ElementId,
    stroke_enabled: Option<bool>,
    stroke_color: Option<String>,
    stroke_width_mm: Option<f64>,
    fill_enabled: Option<bool>,
    fill_color: Option<String>,
    text_color: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ElementAppearanceDto {
    stroke_applicable: bool,
    stroke_enabled: bool,
    stroke_color: String,
    stroke_width_mm: f64,
    fill_applicable: bool,
    fill_enabled: bool,
    fill_color: String,
    text_color_applicable: bool,
    text_color: String,
}

'''
    text = replace_once(
        text,
        "#[derive(Debug, Serialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct ElementEditResultDto {",
        dto + "#[derive(Debug, Serialize)]\n#[serde(rename_all = \"camelCase\")]\nstruct ElementEditResultDto {",
        "appearance DTOs",
    )
    text = replace_once(
        text,
        "    geometry_editable: bool,\n    connector: Option<ConnectorPropertiesDto>,",
        "    geometry_editable: bool,\n    appearance: ElementAppearanceDto,\n    connector: Option<ConnectorPropertiesDto>,",
        "appearance property field",
    )
    text = replace_once(
        text,
        "        Some(element_properties_dto(element, connector))",
        "        Some(element_properties_dto(element, connector, session.document()))",
        "selection appearance call",
    )
    text = replace_once(
        text,
        "        BasicElementKind::Text => (\n            \"Text\".to_owned(),",
        "        BasicElementKind::Ellipse => (\n            \"Ellipse\".to_owned(),\n            40.0,\n            25.0,\n            ElementKind::Ellipse,\n            None,\n        ),\n        BasicElementKind::Text => (\n            \"Text\".to_owned(),",
        "ellipse creation",
    )
    command = r'''
#[tauri::command]
fn update_element_appearance(
    request: UpdateElementAppearanceRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let (stroke_applicable, fill_applicable, text_color_applicable, mut stroke, mut fill, mut text_color) = {
        let session = document.session.session();
        let element = find_element(session.document(), request.element_id).ok_or_else(|| {
            CommandError::new(
                "element_appearance_missing",
                "The selected element no longer exists in the current document.",
            )
        })?;
        let (stroke_applicable, fill_applicable, text_color_applicable) =
            appearance_applicability(&element.kind);
        let (stroke, fill, text_color) = materialized_element_appearance(element, session.document());
        (
            stroke_applicable,
            fill_applicable,
            text_color_applicable,
            stroke,
            fill,
            text_color,
        )
    };

    if (!stroke_applicable
        && (request.stroke_enabled.is_some()
            || request.stroke_color.is_some()
            || request.stroke_width_mm.is_some()))
        || (!fill_applicable && (request.fill_enabled.is_some() || request.fill_color.is_some()))
        || (!text_color_applicable && request.text_color.is_some())
    {
        return Err(CommandError::new(
            "appearance_not_applicable",
            "The requested appearance field does not apply to this element type.",
        ));
    }

    if let Some(enabled) = request.stroke_enabled {
        if enabled {
            stroke.get_or_insert_with(default_stroke);
        } else {
            stroke = None;
        }
    }
    if let Some(width) = request.stroke_width_mm {
        if !width.is_finite() || width <= 0.0 {
            return Err(CommandError::new(
                "invalid_stroke_width",
                "Stroke width must be a finite positive value.",
            ));
        }
        stroke.get_or_insert_with(default_stroke).width_mm = width;
    }
    if let Some(color) = request.stroke_color.as_deref() {
        stroke.get_or_insert_with(default_stroke).color = parse_rgb_color(color)?;
    }

    if let Some(enabled) = request.fill_enabled {
        if enabled {
            fill.get_or_insert_with(default_fill);
        } else {
            fill = None;
        }
    }
    if let Some(color) = request.fill_color.as_deref() {
        let fill = fill.get_or_insert_with(default_fill);
        fill.color = parse_rgb_color(color)?;
        // Choosing a flat colour explicitly replaces an imported gradient.
        fill.gradient = None;
    }
    if let Some(color) = request.text_color.as_deref() {
        text_color = Some(parse_rgb_color(color)?);
    }

    document
        .session
        .set_element_appearance(request.element_id, stroke, fill, text_color)
        .map_err(|error| CommandError::new("element_appearance_failed", error.to_string()))?;
    document
        .session
        .set_selection([request.element_id])
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

'''
    text = replace_once(
        text,
        "#[tauri::command]\nfn new_document(",
        command + "#[tauri::command]\nfn new_document(",
        "appearance command function",
    )
    text = replace_once(
        text,
        "fn element_properties_dto(\n    element: &Element,\n    connector: Option<AppConnectorEndpoints>,\n) -> ElementPropertiesDto {",
        "fn element_properties_dto(\n    element: &Element,\n    connector: Option<AppConnectorEndpoints>,\n    document: &Document,\n) -> ElementPropertiesDto {",
        "appearance dto signature",
    )
    text = replace_once(
        text,
        "        geometry_editable: element_geometry_editable(&element.kind),\n        connector: connector.and_then(connector_properties_dto),",
        "        geometry_editable: element_geometry_editable(&element.kind),\n        appearance: element_appearance_dto(element, document),\n        connector: connector.and_then(connector_properties_dto),",
        "appearance dto construction",
    )
    helpers = r'''
fn appearance_applicability(kind: &ElementKind) -> (bool, bool, bool) {
    let shape = matches!(
        kind,
        ElementKind::Rectangle { .. }
            | ElementKind::Ellipse
            | ElementKind::Polygon { .. }
            | ElementKind::Flowchart { .. }
    );
    let text = matches!(kind, ElementKind::Text);
    (shape, shape, text)
}

fn materialized_element_appearance(
    element: &Element,
    document: &Document,
) -> (Option<StrokeStyle>, Option<FillStyle>, Option<Color>) {
    if let Some(style) = element
        .style_id
        .and_then(|style_id| document.styles.iter().find(|style| style.id == style_id))
    {
        return (style.stroke.clone(), style.fill.clone(), style.text_color);
    }
    let (stroke_applicable, _, text_applicable) = appearance_applicability(&element.kind);
    (
        stroke_applicable.then(default_stroke),
        None,
        text_applicable.then(default_black),
    )
}

fn element_appearance_dto(element: &Element, document: &Document) -> ElementAppearanceDto {
    let (stroke_applicable, fill_applicable, text_color_applicable) =
        appearance_applicability(&element.kind);
    let (stroke, fill, text_color) = materialized_element_appearance(element, document);
    ElementAppearanceDto {
        stroke_applicable,
        stroke_enabled: stroke.is_some(),
        stroke_color: color_to_hex(stroke.as_ref().map(|stroke| stroke.color).unwrap_or_else(default_black)),
        stroke_width_mm: stroke.as_ref().map(|stroke| stroke.width_mm).unwrap_or(0.25),
        fill_applicable,
        fill_enabled: fill.is_some(),
        fill_color: color_to_hex(fill.as_ref().map(|fill| fill.color).unwrap_or(Color::Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        })),
        text_color_applicable,
        text_color: color_to_hex(text_color.unwrap_or_else(default_black)),
    }
}

fn default_black() -> Color {
    Color::Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    }
}

fn default_stroke() -> StrokeStyle {
    StrokeStyle {
        width_mm: 0.25,
        color: default_black(),
    }
}

fn default_fill() -> FillStyle {
    FillStyle {
        color: Color::Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        },
        gradient: None,
    }
}

fn color_to_hex(color: Color) -> String {
    match color {
        Color::Rgba { r, g, b, .. } => format!("#{r:02x}{g:02x}{b:02x}"),
        // System colours are intentionally kept in the domain until the user changes
        // that field. The picker shows the renderer's neutral fallback only.
        Color::SystemPalette { .. } => "#808080".to_owned(),
    }
}

fn parse_rgb_color(value: &str) -> Result<Color, CommandError> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CommandError::new(
            "invalid_color",
            "Colours must use six-digit RGB notation such as #336699.",
        ));
    }
    let parse = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&hex[range], 16)
            .map_err(|_| CommandError::new("invalid_color", "Colour could not be parsed."))
    };
    Ok(Color::Rgba {
        r: parse(0..2)?,
        g: parse(2..4)?,
        b: parse(4..6)?,
        a: 255,
    })
}

'''
    text = replace_once(
        text,
        "fn connector_properties_dto(connector: AppConnectorEndpoints) -> Option<ConnectorPropertiesDto> {",
        helpers + "fn connector_properties_dto(connector: AppConnectorEndpoints) -> Option<ConnectorPropertiesDto> {",
        "appearance helpers",
    )
    text = replace_once(
        text,
        "            update_element_properties,\n            new_document,",
        "            update_element_properties,\n            update_element_appearance,\n            new_document,",
        "tauri handler appearance",
    )
    return text

patch("apps/desktop/src-tauri/src/lib.rs", patch_tauri)


# Tauri build manifest.
def patch_build(text):
    return replace_once(
        text,
        '            "update_element_properties",\n            "new_document",',
        '            "update_element_properties",\n            "update_element_appearance",\n            "new_document",',
        "build command appearance",
    )

patch("apps/desktop/src-tauri/build.rs", patch_build)


# HTML: Ellipse button and appearance inspector.
def patch_html(text):
    text = replace_once(
        text,
        '          <button id="add-rectangle" type="button" title="Create a rectangle on the active layer">Rectangle</button>\n          <button id="add-text"',
        '          <button id="add-rectangle" type="button" title="Create a rectangle on the active layer">Rectangle</button>\n          <button id="add-ellipse" type="button" title="Create an ellipse on the active layer">Ellipse</button>\n          <button id="add-text"',
        "ellipse toolbar",
    )
    appearance = r'''
            <form id="selection-appearance-form" class="appearance-form" hidden>
              <div class="appearance-heading">
                <h3>Appearance</h3>
                <span>basic</span>
              </div>
              <div id="appearance-stroke-section" class="appearance-section">
                <label class="appearance-toggle"><input id="appearance-stroke-enabled" type="checkbox" /> Stroke</label>
                <div class="appearance-row">
                  <label>Color <input id="appearance-stroke-color" type="color" value="#000000" /></label>
                  <label>Width <input id="appearance-stroke-width" type="number" min="0.05" step="0.05" value="0.25" /></label>
                </div>
              </div>
              <div id="appearance-fill-section" class="appearance-section">
                <label class="appearance-toggle"><input id="appearance-fill-enabled" type="checkbox" /> Fill</label>
                <label class="appearance-color-field">Color <input id="appearance-fill-color" type="color" value="#ffffff" /></label>
              </div>
              <label id="appearance-text-color-field" class="appearance-color-field">
                Text color <input id="appearance-text-color" type="color" value="#000000" />
              </label>
              <button id="apply-appearance" type="submit">Apply appearance</button>
            </form>
'''
    text = replace_once(
        text,
        "            </form>\n          </section>\n\n          <p class=\"boundary-note\">",
        "            </form>\n" + appearance + "          </section>\n\n          <p class=\"boundary-note\">",
        "appearance form",
    )
    return text

patch("apps/desktop/ui/index.html", patch_html)


# app.js UI wiring.
def patch_app_js(text):
    text = replace_once(text, "  addRectangle: document.querySelector('#add-rectangle'),\n  addText:", "  addRectangle: document.querySelector('#add-rectangle'),\n  addEllipse: document.querySelector('#add-ellipse'),\n  addText:", "ellipse element")
    text = replace_once(
        text,
        "  applyProperties: document.querySelector('#apply-properties'),\n  rulerX:",
        "  applyProperties: document.querySelector('#apply-properties'),\n  appearanceForm: document.querySelector('#selection-appearance-form'),\n  appearanceStrokeSection: document.querySelector('#appearance-stroke-section'),\n  appearanceStrokeEnabled: document.querySelector('#appearance-stroke-enabled'),\n  appearanceStrokeColor: document.querySelector('#appearance-stroke-color'),\n  appearanceStrokeWidth: document.querySelector('#appearance-stroke-width'),\n  appearanceFillSection: document.querySelector('#appearance-fill-section'),\n  appearanceFillEnabled: document.querySelector('#appearance-fill-enabled'),\n  appearanceFillColor: document.querySelector('#appearance-fill-color'),\n  appearanceTextColorField: document.querySelector('#appearance-text-color-field'),\n  appearanceTextColor: document.querySelector('#appearance-text-color'),\n  applyAppearance: document.querySelector('#apply-appearance'),\n  rulerX:",
        "appearance elements",
    )
    text = replace_once(text, "  elements.addRectangle,\n  elements.addText,", "  elements.addRectangle,\n  elements.addEllipse,\n  elements.addText,", "ellipse actions")
    text = replace_once(text, "  elements.applyProperties,\n  elements.addPage,", "  elements.applyProperties,\n  elements.applyAppearance,\n  elements.addPage,", "appearance action")
    text = replace_once(text, "let currentNavigation = null;\nlet connectorTool", "let currentNavigation = null;\nlet appearanceBaseline = null;\nlet connectorTool", "appearance baseline")
    text = replace_once(
        text,
        "    elements.applyProperties.disabled =\n      !primary || (primary.geometryEditable === false && primary.textEditable !== true);\n    updateStructureDisabledState();",
        "    elements.applyProperties.disabled =\n      !primary || (primary.geometryEditable === false && primary.textEditable !== true);\n    elements.applyAppearance.disabled = !primary?.appearance;\n    updateStructureDisabledState();",
        "appearance busy",
    )
    text = replace_once(text, "  elements.addRectangle.disabled = isBusy || !layerEditable;\n  elements.addText.disabled", "  elements.addRectangle.disabled = isBusy || !layerEditable;\n  elements.addEllipse.disabled = isBusy || !layerEditable;\n  elements.addText.disabled", "ellipse disabled")
    text = replace_once(
        text,
        "  elements.addRectangle.title = layerEditable\n    ? 'Create a rectangle on the active layer'\n    : 'Choose a visible, unlocked layer to create elements';\n  elements.addText.title",
        "  elements.addRectangle.title = layerEditable\n    ? 'Create a rectangle on the active layer'\n    : 'Choose a visible, unlocked layer to create elements';\n  elements.addEllipse.title = layerEditable\n    ? 'Create an ellipse on the active layer'\n    : 'Choose a visible, unlocked layer to create elements';\n  elements.addText.title",
        "ellipse title",
    )
    text = replace_once(
        text,
        "    elements.selectionPropertiesForm.hidden = true;\n    return;",
        "    elements.selectionPropertiesForm.hidden = true;\n    elements.appearanceForm.hidden = true;\n    appearanceBaseline = null;\n    return;",
        "appearance hide",
    )
    text = replace_once(
        text,
        "  if (hasText) {\n    elements.propertyText.value = primary.text;\n    elements.propertyText.disabled = !primary.textEditable;",
        "  renderAppearance(primary.appearance);\n\n  if (hasText) {\n    elements.propertyText.value = primary.text;\n    elements.propertyText.disabled = !primary.textEditable;",
        "render appearance call",
    )
    appearance_functions = r'''
function renderAppearance(appearance) {
  const available = Boolean(
    appearance &&
      (appearance.strokeApplicable || appearance.fillApplicable || appearance.textColorApplicable),
  );
  elements.appearanceForm.hidden = !available;
  elements.applyAppearance.disabled = !available || isBusy;
  if (!available) {
    appearanceBaseline = null;
    return;
  }

  elements.appearanceStrokeSection.hidden = !appearance.strokeApplicable;
  elements.appearanceFillSection.hidden = !appearance.fillApplicable;
  elements.appearanceTextColorField.hidden = !appearance.textColorApplicable;
  elements.appearanceStrokeEnabled.checked = appearance.strokeEnabled;
  elements.appearanceStrokeColor.value = appearance.strokeColor;
  elements.appearanceStrokeWidth.value = String(appearance.strokeWidthMm);
  elements.appearanceFillEnabled.checked = appearance.fillEnabled;
  elements.appearanceFillColor.value = appearance.fillColor;
  elements.appearanceTextColor.value = appearance.textColor;
  appearanceBaseline = Object.freeze({ ...appearance });
  updateAppearanceEnabledState();
}

function updateAppearanceEnabledState() {
  elements.appearanceStrokeColor.disabled = !elements.appearanceStrokeEnabled.checked;
  elements.appearanceStrokeWidth.disabled = !elements.appearanceStrokeEnabled.checked;
  elements.appearanceFillColor.disabled = !elements.appearanceFillEnabled.checked;
}

async function applyAppearance(event) {
  event.preventDefault();
  const primary = currentSelectionProperties?.primary;
  const baseline = appearanceBaseline;
  if (!invoke || !primary || !baseline) {
    return;
  }
  const request = { elementId: primary.elementId };
  if (baseline.strokeApplicable) {
    if (elements.appearanceStrokeEnabled.checked !== baseline.strokeEnabled) {
      request.strokeEnabled = elements.appearanceStrokeEnabled.checked;
    }
    if (elements.appearanceStrokeColor.value.toLowerCase() !== baseline.strokeColor.toLowerCase()) {
      request.strokeColor = elements.appearanceStrokeColor.value;
    }
    const width = Number(elements.appearanceStrokeWidth.value);
    if (!Number.isFinite(width) || width <= 0) {
      setStatus('Stroke width must be a finite positive value');
      return;
    }
    if (width !== baseline.strokeWidthMm) {
      request.strokeWidthMm = width;
    }
  }
  if (baseline.fillApplicable) {
    if (elements.appearanceFillEnabled.checked !== baseline.fillEnabled) {
      request.fillEnabled = elements.appearanceFillEnabled.checked;
    }
    if (elements.appearanceFillColor.value.toLowerCase() !== baseline.fillColor.toLowerCase()) {
      request.fillColor = elements.appearanceFillColor.value;
    }
  }
  if (
    baseline.textColorApplicable &&
    elements.appearanceTextColor.value.toLowerCase() !== baseline.textColor.toLowerCase()
  ) {
    request.textColor = elements.appearanceTextColor.value;
  }
  if (Object.keys(request).length === 1) {
    setStatus('Appearance unchanged');
    return;
  }

  setBusy(true);
  try {
    const result = await invoke('update_element_appearance', { request });
    renderState(result.state);
    await refreshPresentation({ preserveSelection: true });
    const selection = result.selectedElementIds ?? [primary.elementId];
    svgSurface.setSelection(selection);
    keyboardSurface?.syncSelectionState(selection);
    await refreshSelectionProperties();
    scheduleRecoverySync(250);
    setStatus('Appearance updated');
  } catch (error) {
    setStatus(formatInvokeError(error));
  } finally {
    setBusy(false);
  }
}

'''
    text = replace_once(
        text,
        "async function refreshSelectionProperties() {",
        appearance_functions + "async function refreshSelectionProperties() {",
        "appearance functions",
    )
    text = replace_once(
        text,
        "    setStatus(kind === 'text' ? 'Text box created' : 'Rectangle created');",
        "    setStatus(kind === 'text' ? 'Text box created' : kind === 'ellipse' ? 'Ellipse created' : 'Rectangle created');",
        "ellipse status",
    )
    text = replace_once(
        text,
        "elements.addRectangle.addEventListener('click', () => {\n  void createBasicElement('rectangle');\n});\n\nelements.addText",
        "elements.addRectangle.addEventListener('click', () => {\n  void createBasicElement('rectangle');\n});\n\nelements.addEllipse.addEventListener('click', () => {\n  void createBasicElement('ellipse');\n});\n\nelements.addText",
        "ellipse listener",
    )
    text = replace_once(
        text,
        "elements.selectionPropertiesForm.addEventListener('submit', (event) => {\n  void applyElementProperties(event);\n});\n\nelements.saveDocument",
        "elements.selectionPropertiesForm.addEventListener('submit', (event) => {\n  void applyElementProperties(event);\n});\n\nelements.appearanceForm.addEventListener('submit', (event) => {\n  void applyAppearance(event);\n});\nelements.appearanceStrokeEnabled.addEventListener('change', updateAppearanceEnabledState);\nelements.appearanceFillEnabled.addEventListener('change', updateAppearanceEnabledState);\n\nelements.saveDocument",
        "appearance listeners",
    )
    return text

patch("apps/desktop/ui/app.js", patch_app_js)


# CSS can be appended safely.
def patch_css(text):
    block = r'''

/* Basic appearance inspector */
.appearance-form {
  margin-top: 14px;
  padding-top: 14px;
  border-top: 1px solid color-mix(in srgb, currentColor 16%, transparent);
  display: grid;
  gap: 10px;
}

.appearance-heading,
.appearance-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.appearance-heading h3 {
  margin: 0;
  font-size: 0.82rem;
}

.appearance-heading span {
  opacity: 0.55;
  font-size: 0.72rem;
}

.appearance-section {
  display: grid;
  gap: 7px;
}

.appearance-toggle,
.appearance-color-field,
.appearance-row label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 0.78rem;
}

.appearance-row label {
  flex: 1 1 0;
}

.appearance-row input[type="number"] {
  min-width: 0;
  width: 72px;
}

.appearance-form input[type="color"] {
  width: 42px;
  height: 28px;
  padding: 2px;
  border-radius: 6px;
}
'''
    if "/* Basic appearance inspector */" in text:
        raise RuntimeError("appearance CSS already present")
    return text + block

patch("apps/desktop/ui/styles.css", patch_css)
