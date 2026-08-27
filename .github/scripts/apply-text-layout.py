from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one anchor, found {count}: {old[:120]!r}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


# Rust desktop boundary: restricted partial layout DTOs, lossless DTO output,
# protected geometry semantics, and a canonical TextBlock-preserving update path.
rust = "apps/desktop/src-tauri/src/lib.rs"
replace_once(
    rust,
    """struct UpdateElementPropertiesRequest {
    element_id: ElementId,
    bounds_mm: Rect,
    rotation_deg: f64,
    text: Option<String>,
    text_style: Option<TextStyleDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = \"camelCase\")]
struct TextStyleDto {
    bold: bool,
    italic: bool,
    underline: bool,
    font_family: Option<String>,
    font_size_pt: Option<u16>,
}
""",
    """struct UpdateElementPropertiesRequest {
    element_id: ElementId,
    bounds_mm: Rect,
    rotation_deg: f64,
    text: Option<String>,
    text_style: Option<TextStyleDto>,
    text_layout: Option<TextLayoutUpdateDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = \"camelCase\")]
struct TextStyleDto {
    bold: bool,
    italic: bool,
    underline: bool,
    font_family: Option<String>,
    font_size_pt: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = \"camelCase\")]
struct TextLayoutDto {
    horizontal: TextHorizontalAlignment,
    vertical: TextVerticalAlignment,
    margin_mm: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = \"camelCase\")]
struct TextLayoutUpdateDto {
    horizontal: Option<StandardTextHorizontalAlignment>,
    vertical: Option<StandardTextVerticalAlignment>,
    margin_mm: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = \"snake_case\")]
enum StandardTextHorizontalAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = \"snake_case\")]
enum StandardTextVerticalAlignment {
    Top,
    Center,
    Bottom,
}
""",
)
replace_once(
    rust,
    """    text: Option<String>,
    text_editable: bool,
    text_style: Option<TextStyleDto>,
    geometry_editable: bool,
""",
    """    text: Option<String>,
    text_editable: bool,
    text_style: Option<TextStyleDto>,
    text_layout: Option<TextLayoutDto>,
    geometry_editable: bool,
""",
)
replace_once(
    rust,
    """        rotation_deg,
        text,
        text_style,
    } = request;
""",
    """        rotation_deg,
        text,
        text_style,
        text_layout,
    } = request;
""",
)
replace_once(
    rust,
    """    if !element_geometry_editable(&existing.kind) {
        return Err(CommandError::new(
            \"element_geometry_requires_dedicated_tool\",
            \"This element uses a dedicated geometry tool and cannot be resized in the basic inspector.\",
        ));
    }

    let text_update = if text.is_some() || text_style.is_some() {
        let existing =
            find_element(document.session.session().document(), element_id).ok_or_else(|| {
                CommandError::new(
                    \"element_properties_missing\",
                    \"The selected element no longer exists in the current document.\",
                )
            })?;
        let Some(existing_text) = existing.text.as_ref() else {
            return Err(CommandError::new(
                \"element_text_not_editable\",
                \"This element does not contain editable text.\",
            ));
        };
        let (preview, editable, common_style) = text_preview(existing_text);
        if !editable {
            return Err(CommandError::new(
                \"element_text_not_editable\",
                \"This rich-text element cannot be flattened safely by the basic text editor.\",
            ));
        }
        let baseline_style = common_style.unwrap_or_default();
        let next_style = match text_style {
            Some(style) => text_style_from_dto(baseline_style, style)?,
            None => baseline_style,
        };
        let next_text = text.as_deref().unwrap_or(&preview);
        Some(Some(simple_text_block(
            next_text,
            next_style,
            Some(existing_text.layout),
        )))
    } else {
        None
    };
""",
    """    let geometry_changed = bounds_mm != existing.bounds_mm || rotation_deg != existing.rotation_deg;
    if !element_geometry_editable(&existing.kind) && geometry_changed {
        return Err(CommandError::new(
            \"element_geometry_requires_dedicated_tool\",
            \"This element uses a dedicated geometry tool; its bounds and rotation cannot be changed in the basic inspector.\",
        ));
    }

    let text_update = if text.is_some() || text_style.is_some() || text_layout.is_some() {
        let existing =
            find_element(document.session.session().document(), element_id).ok_or_else(|| {
                CommandError::new(
                    \"element_properties_missing\",
                    \"The selected element no longer exists in the current document.\",
                )
            })?;
        let Some(existing_text) = existing.text.as_ref() else {
            return Err(CommandError::new(
                \"element_text_missing\",
                \"This element does not contain a text block.\",
            ));
        };

        let content_or_style_update = text.is_some() || text_style.is_some();
        let (preview, editable, common_style) = text_preview(existing_text);
        if content_or_style_update && !editable {
            return Err(CommandError::new(
                \"element_text_not_editable\",
                \"This rich-text element cannot be flattened safely by the basic text editor.\",
            ));
        }

        let mut next_text_block = if content_or_style_update {
            let baseline_style = common_style.unwrap_or_default();
            let next_style = match text_style {
                Some(style) => text_style_from_dto(baseline_style, style)?,
                None => baseline_style,
            };
            let next_text = text.as_deref().unwrap_or(&preview);
            simple_text_block(next_text, next_style, Some(existing_text.layout))
        } else {
            existing_text.clone()
        };

        if let Some(layout_update) = text_layout {
            next_text_block.layout = apply_text_layout_update(next_text_block.layout, layout_update)?;
        }
        Some(Some(next_text_block))
    } else {
        None
    };
""",
)
replace_once(
    rust,
    """    let (text, text_editable, text_style) = match element.text.as_ref() {
        Some(block) => {
            let (preview, editable, common_style) = text_preview(block);
            let style = editable.then(|| text_style_dto(common_style.unwrap_or_default()));
            (Some(preview), editable, style)
        }
        None => (None, false, None),
    };
""",
    """    let (text, text_editable, text_style, text_layout) = match element.text.as_ref() {
        Some(block) => {
            let (preview, editable, common_style) = text_preview(block);
            let style = editable.then(|| text_style_dto(common_style.unwrap_or_default()));
            (Some(preview), editable, style, Some(text_layout_dto(block.layout)))
        }
        None => (None, false, None, None),
    };
""",
)
replace_once(
    rust,
    """        text,
        text_editable,
        text_style,
        geometry_editable: element_geometry_editable(&element.kind),
""",
    """        text,
        text_editable,
        text_style,
        text_layout,
        geometry_editable: element_geometry_editable(&element.kind),
""",
)
replace_once(
    rust,
    """fn text_preview(block: &TextBlock) -> (String, bool, Option<TextStyle>) {
""",
    """fn text_layout_dto(layout: TextLayout) -> TextLayoutDto {
    TextLayoutDto {
        horizontal: layout.horizontal,
        vertical: layout.vertical,
        margin_mm: layout.margin_mm,
    }
}

fn apply_text_layout_update(
    mut baseline: TextLayout,
    update: TextLayoutUpdateDto,
) -> Result<TextLayout, CommandError> {
    if let Some(horizontal) = update.horizontal {
        baseline.horizontal = match horizontal {
            StandardTextHorizontalAlignment::Left => TextHorizontalAlignment::Left,
            StandardTextHorizontalAlignment::Center => TextHorizontalAlignment::Center,
            StandardTextHorizontalAlignment::Right => TextHorizontalAlignment::Right,
        };
    }
    if let Some(vertical) = update.vertical {
        baseline.vertical = match vertical {
            StandardTextVerticalAlignment::Top => TextVerticalAlignment::Top,
            StandardTextVerticalAlignment::Center => TextVerticalAlignment::Center,
            StandardTextVerticalAlignment::Bottom => TextVerticalAlignment::Bottom,
        };
    }
    if let Some(margin_mm) = update.margin_mm {
        if !margin_mm.is_finite() || margin_mm < 0.0 {
            return Err(CommandError::new(
                \"invalid_text_layout_margin\",
                \"Text inner margin must be a finite non-negative value in millimetres.\",
            ));
        }
        baseline.margin_mm = margin_mm;
    }
    Ok(baseline)
}

fn text_preview(block: &TextBlock) -> (String, bool, Option<TextStyle>) {
""",
)

# Desktop inspector UI and restricted request mapping.
app = "apps/desktop/ui/app.js"
replace_once(
    app,
    """import { buildUniformTextUpdate } from './editor-interaction/text-formatting-actions.mjs';
""",
    """import { buildUniformTextUpdate } from './editor-interaction/text-formatting-actions.mjs';
import {
  buildTextLayoutUpdate,
  textHorizontalChoice,
  textLayoutDisplayMargin,
  textLayoutLegacyLabel,
  textVerticalChoice,
} from './editor-interaction/text-layout-actions.mjs';
""",
)
replace_once(
    app,
    """  propertyTextUnderline: document.querySelector('#property-text-underline'),
  applyProperties: document.querySelector('#apply-properties'),
""",
    """  propertyTextUnderline: document.querySelector('#property-text-underline'),
  propertyTextLayout: document.querySelector('#property-text-layout'),
  propertyTextHorizontal: document.querySelector('#property-text-horizontal'),
  propertyTextVertical: document.querySelector('#property-text-vertical'),
  propertyTextMargin: document.querySelector('#property-text-margin'),
  propertyTextLayoutNote: document.querySelector('#property-text-layout-note'),
  applyProperties: document.querySelector('#apply-properties'),
""",
)
replace_once(
    app,
    """  elements.applyProperties.disabled =
    !primary || (primary.geometryEditable === false && primary.textEditable !== true);
""",
    """  elements.applyProperties.disabled =
    !primary ||
    (primary.geometryEditable === false && primary.textEditable !== true && !primary.textLayout);
""",
)
replace_once(
    app,
    """  const hasText = primary.text !== null && primary.text !== undefined;
  const editableTextStyle = primary.textEditable === true ? primary.textStyle ?? null : null;
  elements.propertyTextField.hidden = !hasText;
  elements.propertyTextNote.hidden = !hasText || primary.textEditable;
  elements.propertyTextFormatting.hidden = !editableTextStyle;
  renderConnectorStyle(primary.connector);
""",
    """  const hasText = primary.text !== null && primary.text !== undefined;
  const editableTextStyle = primary.textEditable === true ? primary.textStyle ?? null : null;
  const textLayout = hasText ? primary.textLayout ?? null : null;
  elements.propertyTextField.hidden = !hasText;
  elements.propertyTextNote.hidden = !hasText || primary.textEditable;
  elements.propertyTextFormatting.hidden = !editableTextStyle;
  elements.propertyTextLayout.hidden = !textLayout;
  renderConnectorStyle(primary.connector);
""",
)
replace_once(
    app,
    """    elements.propertyTextUnderline.checked = editableTextStyle.underline === true;
  }
}


function setConnectorEnumSelect(select, value, label) {
""",
    """    elements.propertyTextUnderline.checked = editableTextStyle.underline === true;
  }
  if (textLayout) {
    setTextLayoutSelect(elements.propertyTextHorizontal, textLayout.horizontal, 'horizontal');
    setTextLayoutSelect(elements.propertyTextVertical, textLayout.vertical, 'vertical');
    elements.propertyTextMargin.value = String(textLayoutDisplayMargin(textLayout.marginMm));
    const importedMarginFallback =
      !Number.isFinite(Number(textLayout.marginMm)) || Number(textLayout.marginMm) < 0;
    elements.propertyTextLayoutNote.textContent = importedMarginFallback
      ? 'Imported inner margin is outside the editable range. The renderer fallback is shown; the original value is preserved until you deliberately change it.'
      : 'Imported special alignments remain preserved until you deliberately select a standard alignment.';
  }
}

function setTextLayoutSelect(select, value, axis) {
  for (const option of [...select.options]) {
    if (option.dataset.textLayoutLegacy === 'true') {
      option.remove();
    }
  }
  const choice = axis === 'horizontal' ? textHorizontalChoice(value) : textVerticalChoice(value);
  if (choice.startsWith('legacy:')) {
    const option = document.createElement('option');
    option.value = choice;
    option.textContent = textLayoutLegacyLabel(value, axis);
    option.dataset.textLayoutLegacy = 'true';
    select.prepend(option);
  }
  select.value = choice;
}

function setConnectorEnumSelect(select, value, label) {
""",
)
replace_once(
    app,
    """  if (primary.geometryEditable === false) {
    setStatus('This element uses a dedicated geometry tool and cannot be resized in the basic inspector');
    return;
  }
""",
    """""",
)
replace_once(
    app,
    """  if (primary.textEditable) {
    try {
      Object.assign(
        request,
        buildUniformTextUpdate({
          baselineText: primary.text,
          baselineStyle: primary.textStyle,
          text: elements.propertyText.value,
          fontFamily: elements.propertyTextFontFamily.value,
          fontSizePt: elements.propertyTextFontSize.value,
          bold: elements.propertyTextBold.checked,
          italic: elements.propertyTextItalic.checked,
          underline: elements.propertyTextUnderline.checked,
        }),
      );
    } catch (error) {
      setStatus(String(error?.message ?? error));
      return;
    }
  }

  setBusy(true);
""",
    """  if (primary.textEditable) {
    try {
      Object.assign(
        request,
        buildUniformTextUpdate({
          baselineText: primary.text,
          baselineStyle: primary.textStyle,
          text: elements.propertyText.value,
          fontFamily: elements.propertyTextFontFamily.value,
          fontSizePt: elements.propertyTextFontSize.value,
          bold: elements.propertyTextBold.checked,
          italic: elements.propertyTextItalic.checked,
          underline: elements.propertyTextUnderline.checked,
        }),
      );
    } catch (error) {
      setStatus(String(error?.message ?? error));
      return;
    }
  }
  if (primary.textLayout) {
    try {
      const textLayout = buildTextLayoutUpdate({
        baseline: primary.textLayout,
        horizontalChoice: elements.propertyTextHorizontal.value,
        verticalChoice: elements.propertyTextVertical.value,
        marginMm: elements.propertyTextMargin.value,
      });
      if (textLayout) {
        request.textLayout = textLayout;
      }
    } catch (error) {
      setStatus(String(error?.message ?? error));
      return;
    }
  }

  setBusy(true);
""",
)

html = "apps/desktop/ui/index.html"
replace_once(
    html,
    """                <p class=\"property-note\">Blank family or size uses the document default. Other imported text-style semantics are preserved.</p>
              </section>
              <button id=\"apply-properties\" class=\"primary\" type=\"submit\">Apply properties</button>
""",
    """                <p class=\"property-note\">Blank family or size uses the document default. Other imported text-style semantics are preserved.</p>
              </section>
              <section id=\"property-text-layout\" class=\"text-formatting-section\" aria-label=\"Text block layout\" hidden>
                <div class=\"text-formatting-heading\">
                  <strong>Text layout</strong>
                  <span>block geometry</span>
                </div>
                <div class=\"text-layout-grid\">
                  <label class=\"property-field\">Horizontal
                    <select id=\"property-text-horizontal\">
                      <option value=\"left\">Left</option>
                      <option value=\"center\">Center</option>
                      <option value=\"right\">Right</option>
                    </select>
                  </label>
                  <label class=\"property-field\">Vertical
                    <select id=\"property-text-vertical\">
                      <option value=\"top\">Top</option>
                      <option value=\"center\">Center</option>
                      <option value=\"bottom\">Bottom</option>
                    </select>
                  </label>
                  <label class=\"property-field text-layout-margin\">Inner margin (mm)
                    <input id=\"property-text-margin\" type=\"number\" min=\"0\" step=\"0.1\" />
                  </label>
                </div>
                <p id=\"property-text-layout-note\" class=\"property-note\"></p>
              </section>
              <button id=\"apply-properties\" class=\"primary\" type=\"submit\">Apply properties</button>
""",
)

css = "apps/desktop/ui/styles.css"
replace_once(
    css,
    """.text-formatting-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 82px;
  gap: 8px;
}

.text-formatting-toggles {
""",
    """.text-formatting-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 82px;
  gap: 8px;
}

.text-layout-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  margin-bottom: 8px;
}

.text-layout-margin {
  grid-column: 1 / -1;
}

.text-formatting-toggles {
""",
)

# Extend the existing app-core regression with combined layout and protected-rich-text coverage.
test_path = "crates/app-core/tests/text_properties_application.rs"
replace_once(
    test_path,
    """    RichTextToken, Scene, ScriptPosition, Size, TextBlock, TextHorizontalAlignment, TextLayout,
    TextStyle, TextVerticalAlignment,
""",
    """    RichTextToken, Scene, ScriptPosition, Size, TextBlock, TextHorizontalAlignment, TextLayout,
    TextStyle, TextTailDirective, TextTailKind, TextVerticalAlignment,
""",
)
replace_once(
    test_path,
    """    let updated = text_block(\"Beta\", updated_style());

    assert!(
""",
    """    let mut updated = text_block(\"Beta\", updated_style());
    updated.layout = TextLayout {
        horizontal: TextHorizontalAlignment::Center,
        vertical: TextVerticalAlignment::Bottom,
        margin_mm: 3.25,
    };

    assert!(
""",
)
append = r'''

#[test]
fn layout_only_update_preserves_protected_rich_text_and_survives_ddnx() {
    let (artifact, element_id, _) = fixture();
    let mut app = ApplicationSession::from_artifact(artifact).unwrap();
    let protected = TextBlock {
        content: RichTextDocument {
            tokens: vec![
                RichTextToken::Text {
                    text: "Page ".to_owned(),
                    style: initial_style(),
                },
                RichTextToken::PageNumber {
                    style: updated_style(),
                },
                RichTextToken::NewLine,
                RichTextToken::SymbolGlyph {
                    legacy_glyph: 'x',
                    style: TextStyle {
                        symbol_font: true,
                        ..initial_style()
                    },
                },
            ],
            tail: Some(TextTailDirective {
                kind: TextTailKind::Hint,
                value: "legacy-hint".to_owned(),
            }),
            diagnostics: vec!["preserve imported diagnostic".to_owned()],
        },
        layout: TextLayout {
            horizontal: TextHorizontalAlignment::BlockRight,
            vertical: TextVerticalAlignment::LegacyUnknown(-3),
            margin_mm: 0.75,
        },
    };
    assert!(
        app.commit_element_properties(
            element_id,
            bounds(),
            0.0,
            Some(Some(protected.clone())),
        )
        .unwrap()
    );
    let protected_history = app.session().current_history_state();

    let mut layout_only = protected.clone();
    layout_only.layout = TextLayout {
        horizontal: TextHorizontalAlignment::Right,
        vertical: TextVerticalAlignment::Center,
        margin_mm: 2.5,
    };
    assert!(
        app.commit_element_properties(
            element_id,
            bounds(),
            0.0,
            Some(Some(layout_only.clone())),
        )
        .unwrap()
    );
    let layout_history = app.session().current_history_state();
    assert_ne!(layout_history, protected_history);
    assert_eq!(block(&app, element_id).content, protected.content);
    assert_eq!(block(&app, element_id).layout, layout_only.layout);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened =
        ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(block(&reopened, element_id), &layout_only);
    assert_eq!(block(&reopened, element_id).content, protected.content);

    assert!(app.undo().unwrap());
    assert_eq!(app.session().current_history_state(), protected_history);
    assert_eq!(block(&app, element_id), &protected);

    assert!(app.redo().unwrap());
    assert_eq!(app.session().current_history_state(), layout_history);
    assert_eq!(block(&app, element_id), &layout_only);
}
'''
path = Path(test_path)
text = path.read_text(encoding="utf-8")
if "fn layout_only_update_preserves_protected_rich_text_and_survives_ddnx()" in text:
    raise SystemExit(f"{test_path}: layout regression already exists")
path.write_text(text.rstrip() + append + "\n", encoding="utf-8")
