from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one anchor, found {count}: {old[:80]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


LIB = "apps/desktop/src-tauri/src/lib.rs"
APP = "apps/desktop/ui/app.js"
HTML = "apps/desktop/ui/index.html"
CSS = "apps/desktop/ui/styles.css"
DOC = "docs/testing/alpha-0.1.md"

replace_once(
    LIB,
    "    Port, PortId, Rect, RichTextDocument, RichTextToken, Scene, Size, StrokeStyle, StyleId,\n    TextBlock, TextHorizontalAlignment, TextLayout, TextStyle, TextVerticalAlignment,\n",
    "    Port, PortId, Rect, RichTextDocument, RichTextToken, Scene, ScriptPosition, Size, StrokeStyle,\n    StyleId, TextBlock, TextHorizontalAlignment, TextLayout, TextStyle, TextVerticalAlignment,\n",
)

replace_once(
    LIB,
    '''struct UpdateElementPropertiesRequest {\n    element_id: ElementId,\n    bounds_mm: Rect,\n    rotation_deg: f64,\n    text: Option<String>,\n}\n\n#[derive(Debug, Deserialize)]\n''',
    '''struct UpdateElementPropertiesRequest {\n    element_id: ElementId,\n    bounds_mm: Rect,\n    rotation_deg: f64,\n    text: Option<String>,\n    text_style: Option<TextStyleDto>,\n}\n\n#[derive(Debug, Clone, Serialize, Deserialize)]\n#[serde(rename_all = "camelCase")]\nstruct TextStyleDto {\n    bold: bool,\n    italic: bool,\n    underline: bool,\n    strikeout: bool,\n    script: ScriptPosition,\n    overline: bool,\n    symbol_font: bool,\n    font_family: Option<String>,\n    font_size_pt: Option<u16>,\n    color: Option<Color>,\n}\n\n#[derive(Debug, Deserialize)]\n''',
)

replace_once(
    LIB,
    '''    text: Option<String>,\n    text_editable: bool,\n    geometry_editable: bool,\n''',
    '''    text: Option<String>,\n    text_editable: bool,\n    text_style: Option<TextStyleDto>,\n    geometry_editable: bool,\n''',
)

replace_once(
    LIB,
    '''#[tauri::command]\nfn update_element_properties(\n    request: UpdateElementPropertiesRequest,\n    state: State<'_, DesktopState>,\n) -> Result<ElementEditResultDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let existing = find_element(document.session.session().document(), request.element_id)\n        .ok_or_else(|| {\n            CommandError::new(\n                "element_properties_missing",\n                "The selected element no longer exists in the current document.",\n            )\n        })?;\n    if !element_geometry_editable(&existing.kind) {\n        return Err(CommandError::new(\n            "element_geometry_requires_dedicated_tool",\n            "This element uses a dedicated geometry tool and cannot be resized in the basic inspector.",\n        ));\n    }\n    let text_update = if let Some(text) = request.text {\n        let existing = find_element(document.session.session().document(), request.element_id)\n            .ok_or_else(|| {\n                CommandError::new(\n                    "element_properties_missing",\n                    "The selected element no longer exists in the current document.",\n                )\n            })?;\n        let Some(existing_text) = existing.text.as_ref() else {\n            return Err(CommandError::new(\n                "element_text_not_editable",\n                "This element does not contain editable text.",\n            ));\n        };\n        let (_, editable, common_style) = text_preview(existing_text);\n        if !editable {\n            return Err(CommandError::new(\n                "element_text_not_editable",\n                "This rich-text element cannot be flattened safely by the basic text editor.",\n            ));\n        }\n        Some(Some(simple_text_block(\n            &text,\n            common_style.unwrap_or_default(),\n            Some(existing_text.layout),\n        )))\n    } else {\n        None\n    };\n\n    document\n        .session\n        .commit_element_properties(\n            request.element_id,\n            request.bounds_mm,\n            request.rotation_deg,\n            text_update,\n        )\n        .map_err(|error| CommandError::new("element_properties_failed", error.to_string()))?;\n    Ok(element_edit_result_dto(&document))\n}\n''',
    '''#[tauri::command]\nfn update_element_properties(\n    request: UpdateElementPropertiesRequest,\n    state: State<'_, DesktopState>,\n) -> Result<ElementEditResultDto, CommandError> {\n    let UpdateElementPropertiesRequest {\n        element_id,\n        bounds_mm,\n        rotation_deg,\n        text,\n        text_style,\n    } = request;\n    let mut document = lock_document(&state)?;\n    let existing = find_element(document.session.session().document(), element_id).ok_or_else(|| {\n        CommandError::new(\n            "element_properties_missing",\n            "The selected element no longer exists in the current document.",\n        )\n    })?;\n    if !element_geometry_editable(&existing.kind) {\n        return Err(CommandError::new(\n            "element_geometry_requires_dedicated_tool",\n            "This element uses a dedicated geometry tool and cannot be resized in the basic inspector.",\n        ));\n    }\n\n    let text_update = if text.is_some() || text_style.is_some() {\n        let existing = find_element(document.session.session().document(), element_id).ok_or_else(|| {\n            CommandError::new(\n                "element_properties_missing",\n                "The selected element no longer exists in the current document.",\n            )\n        })?;\n        let Some(existing_text) = existing.text.as_ref() else {\n            return Err(CommandError::new(\n                "element_text_not_editable",\n                "This element does not contain editable text.",\n            ));\n        };\n        let (preview, editable, common_style) = text_preview(existing_text);\n        if !editable {\n            return Err(CommandError::new(\n                "element_text_not_editable",\n                "This rich-text element cannot be flattened safely by the basic text editor.",\n            ));\n        }\n        let next_style = match text_style {\n            Some(style) => text_style_from_dto(style)?,\n            None => common_style.unwrap_or_default(),\n        };\n        let next_text = text.as_deref().unwrap_or(&preview);\n        Some(Some(simple_text_block(\n            next_text,\n            next_style,\n            Some(existing_text.layout),\n        )))\n    } else {\n        None\n    };\n\n    document\n        .session\n        .commit_element_properties(element_id, bounds_mm, rotation_deg, text_update)\n        .map_err(|error| CommandError::new("element_properties_failed", error.to_string()))?;\n    Ok(element_edit_result_dto(&document))\n}\n''',
)

replace_once(
    LIB,
    '''    let (text, text_editable) = match element.text.as_ref() {\n        Some(block) => {\n            let (preview, editable, _) = text_preview(block);\n            (Some(preview), editable)\n        }\n        None => (None, false),\n    };\n''',
    '''    let (text, text_editable, text_style) = match element.text.as_ref() {\n        Some(block) => {\n            let (preview, editable, common_style) = text_preview(block);\n            let style = editable.then(|| text_style_dto(common_style.unwrap_or_default()));\n            (Some(preview), editable, style)\n        }\n        None => (None, false, None),\n    };\n''',
)

replace_once(
    LIB,
    '''        text,\n        text_editable,\n        geometry_editable: element_geometry_editable(&element.kind),\n''',
    '''        text,\n        text_editable,\n        text_style,\n        geometry_editable: element_geometry_editable(&element.kind),\n''',
)

replace_once(
    LIB,
    '''fn text_preview(block: &TextBlock) -> (String, bool, Option<TextStyle>) {\n''',
    '''fn text_style_dto(style: TextStyle) -> TextStyleDto {\n    TextStyleDto {\n        bold: style.bold,\n        italic: style.italic,\n        underline: style.underline,\n        strikeout: style.strikeout,\n        script: style.script,\n        overline: style.overline,\n        symbol_font: style.symbol_font,\n        font_family: style.font_family,\n        font_size_pt: style.font_size_pt,\n        color: style.color,\n    }\n}\n\nfn text_style_from_dto(style: TextStyleDto) -> Result<TextStyle, CommandError> {\n    if style.font_size_pt == Some(0) {\n        return Err(CommandError::new(\n            "invalid_text_font_size",\n            "Text font size must be a positive whole number of points.",\n        ));\n    }\n    let font_family = style.font_family.and_then(|family| {\n        let trimmed = family.trim();\n        (!trimmed.is_empty()).then(|| trimmed.to_owned())\n    });\n    Ok(TextStyle {\n        bold: style.bold,\n        italic: style.italic,\n        underline: style.underline,\n        strikeout: style.strikeout,\n        script: style.script,\n        overline: style.overline,\n        symbol_font: style.symbol_font,\n        font_family,\n        font_size_pt: style.font_size_pt,\n        color: style.color,\n    })\n}\n\nfn text_preview(block: &TextBlock) -> (String, bool, Option<TextStyle>) {\n''',
)

replace_once(
    APP,
    '''import {\n  appearanceControlState,\n  buildAppearanceRequest,\n} from './editor-interaction/appearance-actions.mjs';\n''',
    '''import {\n  appearanceControlState,\n  buildAppearanceRequest,\n} from './editor-interaction/appearance-actions.mjs';\nimport { buildUniformTextUpdate } from './editor-interaction/text-formatting-actions.mjs';\n''',
)

replace_once(
    APP,
    '''  propertyText: document.querySelector('#property-text'),\n  propertyTextNote: document.querySelector('#property-text-note'),\n  applyProperties: document.querySelector('#apply-properties'),\n''',
    '''  propertyText: document.querySelector('#property-text'),\n  propertyTextNote: document.querySelector('#property-text-note'),\n  propertyTextFormatting: document.querySelector('#property-text-formatting'),\n  propertyTextFontFamily: document.querySelector('#property-text-font-family'),\n  propertyTextFontSize: document.querySelector('#property-text-font-size'),\n  propertyTextBold: document.querySelector('#property-text-bold'),\n  propertyTextItalic: document.querySelector('#property-text-italic'),\n  propertyTextUnderline: document.querySelector('#property-text-underline'),\n  applyProperties: document.querySelector('#apply-properties'),\n''',
)

replace_once(
    APP,
    '''  const hasText = primary.text !== null && primary.text !== undefined;\n  elements.propertyTextField.hidden = !hasText;\n  elements.propertyTextNote.hidden = !hasText || primary.textEditable;\n  renderConnectorStyle(primary.connector);\n  renderAppearance(primary.appearance);\n\n  if (hasText) {\n    elements.propertyText.value = primary.text;\n    elements.propertyText.disabled = !primary.textEditable;\n    if (!primary.textEditable) {\n      elements.propertyTextNote.textContent =\n        'Rich text is shown for reference; this basic editor will not flatten mixed formatting or dynamic fields.';\n    }\n  }\n''',
    '''  const hasText = primary.text !== null && primary.text !== undefined;\n  const editableTextStyle = primary.textEditable === true ? primary.textStyle ?? null : null;\n  elements.propertyTextField.hidden = !hasText;\n  elements.propertyTextNote.hidden = !hasText || primary.textEditable;\n  elements.propertyTextFormatting.hidden = !editableTextStyle;\n  renderConnectorStyle(primary.connector);\n  renderAppearance(primary.appearance);\n\n  if (hasText) {\n    elements.propertyText.value = primary.text;\n    elements.propertyText.disabled = !primary.textEditable;\n    if (!primary.textEditable) {\n      elements.propertyTextNote.textContent =\n        'Rich text is shown for reference; this basic editor will not flatten mixed formatting or dynamic fields.';\n    }\n  }\n  if (editableTextStyle) {\n    elements.propertyTextFontFamily.value = editableTextStyle.fontFamily ?? '';\n    elements.propertyTextFontSize.value =\n      editableTextStyle.fontSizePt === null || editableTextStyle.fontSizePt === undefined\n        ? ''\n        : String(editableTextStyle.fontSizePt);\n    elements.propertyTextBold.checked = editableTextStyle.bold === true;\n    elements.propertyTextItalic.checked = editableTextStyle.italic === true;\n    elements.propertyTextUnderline.checked = editableTextStyle.underline === true;\n  }\n''',
)

replace_once(
    APP,
    '''  if (primary.textEditable) {\n    request.text = elements.propertyText.value;\n  }\n\n  setBusy(true);\n''',
    '''  if (primary.textEditable) {\n    try {\n      Object.assign(\n        request,\n        buildUniformTextUpdate({\n          baselineText: primary.text,\n          baselineStyle: primary.textStyle,\n          text: elements.propertyText.value,\n          fontFamily: elements.propertyTextFontFamily.value,\n          fontSizePt: elements.propertyTextFontSize.value,\n          bold: elements.propertyTextBold.checked,\n          italic: elements.propertyTextItalic.checked,\n          underline: elements.propertyTextUnderline.checked,\n        }),\n      );\n    } catch (error) {\n      setStatus(String(error?.message ?? error));\n      return;\n    }\n  }\n\n  setBusy(true);\n''',
)

replace_once(
    HTML,
    '''              <p id="property-text-note" class="property-note" hidden></p>\n              <button id="apply-properties" class="primary" type="submit">Apply properties</button>\n''',
    '''              <p id="property-text-note" class="property-note" hidden></p>\n              <section id="property-text-formatting" class="text-formatting-section" aria-label="Uniform text formatting" hidden>\n                <div class="text-formatting-heading">\n                  <strong>Text formatting</strong>\n                  <span>uniform block</span>\n                </div>\n                <div class="text-formatting-grid">\n                  <label class="property-field">Font family\n                    <input id="property-text-font-family" type="text" placeholder="Document default" />\n                  </label>\n                  <label class="property-field">Size (pt)\n                    <input id="property-text-font-size" type="number" min="1" max="65535" step="1" placeholder="Default" />\n                  </label>\n                </div>\n                <div class="text-formatting-toggles" role="group" aria-label="Text emphasis">\n                  <label><input id="property-text-bold" type="checkbox" /> Bold</label>\n                  <label><input id="property-text-italic" type="checkbox" /> Italic</label>\n                  <label><input id="property-text-underline" type="checkbox" /> Underline</label>\n                </div>\n                <p class="property-note">Blank family or size uses the document default. Other imported text-style semantics are preserved.</p>\n              </section>\n              <button id="apply-properties" class="primary" type="submit">Apply properties</button>\n''',
)

replace_once(
    CSS,
    '''.property-grid input,\n.property-field input,\n.property-field textarea {\n''',
    '''.property-grid input,\n.property-field input,\n.property-field select,\n.property-field textarea {\n''',
)

replace_once(
    CSS,
    '''.property-note {\n  margin: 0;\n  line-height: 1.35;\n}\n\n.document-structure {\n''',
    '''.property-note {\n  margin: 0;\n  line-height: 1.35;\n}\n\n.text-formatting-section {\n  padding: 10px;\n  border: 1px solid var(--border);\n  border-radius: 9px;\n  background: var(--surface-subtle);\n}\n\n.text-formatting-heading {\n  display: flex;\n  align-items: baseline;\n  justify-content: space-between;\n  gap: 8px;\n  margin-bottom: 8px;\n  font-size: 0.78rem;\n}\n\n.text-formatting-heading span {\n  color: var(--muted);\n  font-size: 0.7rem;\n}\n\n.text-formatting-grid {\n  display: grid;\n  grid-template-columns: minmax(0, 1fr) 82px;\n  gap: 8px;\n}\n\n.text-formatting-toggles {\n  display: flex;\n  flex-wrap: wrap;\n  gap: 10px;\n  margin: 9px 0;\n  color: var(--muted);\n  font-size: 0.74rem;\n}\n\n.text-formatting-toggles label {\n  display: inline-flex;\n  align-items: center;\n  gap: 5px;\n}\n\n.text-formatting-toggles input {\n  width: auto !important;\n  min-height: auto !important;\n  margin: 0;\n  padding: 0 !important;\n}\n\n.document-structure {\n''',
)

replace_once(
    CSS,
    '''.document-structure select,\n.document-structure input,\n.selection-inspector input,\n.selection-inspector textarea {\n''',
    '''.document-structure select,\n.document-structure input,\n.selection-inspector input,\n.selection-inspector select,\n.selection-inspector textarea {\n''',
)

replace_once(
    CSS,
    '''.document-structure select,\n.document-structure input,\n.selection-inspector input {\n''',
    '''.document-structure select,\n.document-structure input,\n.selection-inspector input,\n.selection-inspector select {\n''',
)

replace_once(
    CSS,
    '''.document-structure select:focus-visible,\n.document-structure input:focus-visible,\n.selection-inspector input:focus-visible,\n.selection-inspector textarea:focus-visible {\n''',
    '''.document-structure select:focus-visible,\n.document-structure input:focus-visible,\n.selection-inspector input:focus-visible,\n.selection-inspector select:focus-visible,\n.selection-inspector textarea:focus-visible {\n''',
)

replace_once(
    DOC,
    '''- linear shape fill gradients with independent start/end colours and horizontal/vertical direction, using the existing one-step appearance history and DDNX style model.\n''',
    '''- linear shape fill gradients with independent start/end colours and horizontal/vertical direction, using the existing one-step appearance history and DDNX style model;\n- uniform text formatting for safely editable text blocks: font family, whole-point font size, bold, italic and underline, committed atomically with text edits while preserving all unexposed `TextStyle` fields.\n''',
)

replace_once(
    DOC,
    '''- Basic appearance covers shape stroke/fill including linear gradients, text colour, and standard connector markers/line styles. Advanced text formatting and direct editing of arbitrary custom legacy connector style codes remain outside the current controls.\n''',
    '''- Basic appearance covers shape stroke/fill including linear gradients, text colour, and standard connector markers/line styles. Uniform simple-text family/size/bold/italic/underline are editable; mixed/run-level rich-text formatting and direct editing of arbitrary custom legacy connector style codes remain outside the current controls.\n''',
)

print("uniform text formatting source integration applied")
