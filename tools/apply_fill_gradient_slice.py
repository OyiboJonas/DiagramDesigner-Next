from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one anchor, found {count}: {old[:80]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


TAURI = "apps/desktop/src-tauri/src/lib.rs"
APP = "apps/desktop/ui/app.js"
HTML = "apps/desktop/ui/index.html"
DOC = "docs/testing/alpha-0.1.md"

replace_once(
    TAURI,
    """    DocumentId, Element, ElementId, ElementKind, Endpoint, FillStyle, Layer, LayerId, LineStyle,\n    MarkerStyle, NextArtifact, NormalizedPoint, Page, PageId, Point, Port, PortId, Rect,\n""",
    """    DocumentId, Element, ElementId, ElementKind, Endpoint, FillStyle, GradientAxis, Layer, LayerId,\n    LineStyle, LinearGradient, MarkerStyle, NextArtifact, NormalizedPoint, Page, PageId, Point,\n    Port, PortId, Rect,\n""",
)

replace_once(
    TAURI,
    """struct UpdateElementAppearanceRequest {\n    element_id: ElementId,\n    stroke_enabled: Option<bool>,\n    stroke_color: Option<String>,\n    stroke_width_mm: Option<f64>,\n    fill_enabled: Option<bool>,\n    fill_color: Option<String>,\n    text_color: Option<String>,\n}\n""",
    """struct UpdateElementAppearanceRequest {\n    element_id: ElementId,\n    stroke_enabled: Option<bool>,\n    stroke_color: Option<String>,\n    stroke_width_mm: Option<f64>,\n    fill_enabled: Option<bool>,\n    fill_color: Option<String>,\n    fill_gradient_enabled: Option<bool>,\n    fill_gradient_end_color: Option<String>,\n    fill_gradient_axis: Option<GradientAxis>,\n    text_color: Option<String>,\n}\n""",
)

replace_once(
    TAURI,
    """struct ElementAppearanceDto {\n    stroke_applicable: bool,\n    stroke_enabled: bool,\n    stroke_color: String,\n    stroke_width_mm: f64,\n    fill_applicable: bool,\n    fill_enabled: bool,\n    fill_color: String,\n    text_color_applicable: bool,\n    text_color: String,\n}\n""",
    """struct ElementAppearanceDto {\n    stroke_applicable: bool,\n    stroke_enabled: bool,\n    stroke_color: String,\n    stroke_width_mm: f64,\n    fill_applicable: bool,\n    fill_enabled: bool,\n    fill_color: String,\n    fill_gradient_enabled: bool,\n    fill_gradient_end_color: String,\n    fill_gradient_axis: GradientAxis,\n    text_color_applicable: bool,\n    text_color: String,\n}\n""",
)

replace_once(
    TAURI,
    """        || (!fill_applicable && (request.fill_enabled.is_some() || request.fill_color.is_some()))\n        || (!text_color_applicable && request.text_color.is_some())\n""",
    """        || (!fill_applicable\n            && (request.fill_enabled.is_some()\n                || request.fill_color.is_some()\n                || request.fill_gradient_enabled.is_some()\n                || request.fill_gradient_end_color.is_some()\n                || request.fill_gradient_axis.is_some()))\n        || (!text_color_applicable && request.text_color.is_some())\n""",
)

replace_once(
    TAURI,
    """    if let Some(enabled) = request.stroke_enabled {\n""",
    """    if request.fill_enabled == Some(false)\n        && (request.fill_color.is_some()\n            || request.fill_gradient_enabled.is_some()\n            || request.fill_gradient_end_color.is_some()\n            || request.fill_gradient_axis.is_some())\n    {\n        return Err(CommandError::new(\n            \"appearance_fill_disabled_details\",\n            \"Fill detail fields cannot be changed while fill is being disabled.\",\n        ));\n    }\n    if request.fill_gradient_enabled == Some(false)\n        && (request.fill_gradient_end_color.is_some() || request.fill_gradient_axis.is_some())\n    {\n        return Err(CommandError::new(\n            \"appearance_gradient_disabled_details\",\n            \"Gradient detail fields cannot be changed while the gradient is being disabled.\",\n        ));\n    }\n\n    if let Some(enabled) = request.stroke_enabled {\n""",
)

replace_once(
    TAURI,
    """    if let Some(color) = request.fill_color.as_deref() {\n        let fill = fill.get_or_insert_with(default_fill);\n        fill.color = parse_rgb_color(color)?;\n        // Choosing a flat colour explicitly replaces an imported gradient.\n        fill.gradient = None;\n    }\n    if let Some(color) = request.text_color.as_deref() {\n""",
    """    if let Some(color) = request.fill_color.as_deref() {\n        fill.get_or_insert_with(default_fill).color = parse_rgb_color(color)?;\n    }\n    if let Some(enabled) = request.fill_gradient_enabled {\n        if enabled {\n            let fill = fill.get_or_insert_with(default_fill);\n            if fill.gradient.is_none() {\n                fill.gradient = Some(default_linear_gradient(fill.color));\n            }\n        } else if let Some(fill) = fill.as_mut() {\n            fill.gradient = None;\n        }\n    }\n    if let Some(color) = request.fill_gradient_end_color.as_deref() {\n        let gradient = fill\n            .as_mut()\n            .and_then(|fill| fill.gradient.as_mut())\n            .ok_or_else(|| {\n                CommandError::new(\n                    \"appearance_gradient_missing\",\n                    \"Enable fill and its linear gradient before changing the gradient end colour.\",\n                )\n            })?;\n        gradient.end_color = parse_rgb_color(color)?;\n    }\n    if let Some(axis) = request.fill_gradient_axis {\n        let gradient = fill\n            .as_mut()\n            .and_then(|fill| fill.gradient.as_mut())\n            .ok_or_else(|| {\n                CommandError::new(\n                    \"appearance_gradient_missing\",\n                    \"Enable fill and its linear gradient before changing the gradient axis.\",\n                )\n            })?;\n        gradient.axis = axis;\n    }\n    if let Some(color) = request.text_color.as_deref() {\n""",
)

replace_once(
    TAURI,
    """fn element_appearance_dto(element: &Element, document: &Document) -> ElementAppearanceDto {\n    let (stroke_applicable, fill_applicable, text_color_applicable) =\n        appearance_applicability(&element.kind);\n    let (stroke, fill, text_color) = materialized_element_appearance(element, document);\n    ElementAppearanceDto {\n        stroke_applicable,\n        stroke_enabled: stroke.is_some(),\n        stroke_color: color_to_hex(\n            stroke\n                .as_ref()\n                .map(|stroke| stroke.color)\n                .unwrap_or_else(default_black),\n        ),\n        stroke_width_mm: stroke\n            .as_ref()\n            .map(|stroke| stroke.width_mm)\n            .unwrap_or(0.25),\n        fill_applicable,\n        fill_enabled: fill.is_some(),\n        fill_color: color_to_hex(fill.as_ref().map(|fill| fill.color).unwrap_or(Color::Rgba {\n            r: 255,\n            g: 255,\n            b: 255,\n            a: 255,\n        })),\n        text_color_applicable,\n        text_color: color_to_hex(text_color.unwrap_or_else(default_black)),\n    }\n}\n""",
    """fn element_appearance_dto(element: &Element, document: &Document) -> ElementAppearanceDto {\n    let (stroke_applicable, fill_applicable, text_color_applicable) =\n        appearance_applicability(&element.kind);\n    let (stroke, fill, text_color) = materialized_element_appearance(element, document);\n    let fallback_fill_color = Color::Rgba {\n        r: 255,\n        g: 255,\n        b: 255,\n        a: 255,\n    };\n    let displayed_fill_color = fill\n        .as_ref()\n        .map(|fill| fill.color)\n        .unwrap_or(fallback_fill_color);\n    let (fill_gradient_enabled, fill_gradient_end_color, fill_gradient_axis) = fill\n        .as_ref()\n        .and_then(|fill| fill.gradient.as_ref())\n        .map(|gradient| (true, color_to_hex(gradient.end_color), gradient.axis))\n        .unwrap_or_else(|| (false, color_to_hex(displayed_fill_color), GradientAxis::AlongX));\n    ElementAppearanceDto {\n        stroke_applicable,\n        stroke_enabled: stroke.is_some(),\n        stroke_color: color_to_hex(\n            stroke\n                .as_ref()\n                .map(|stroke| stroke.color)\n                .unwrap_or_else(default_black),\n        ),\n        stroke_width_mm: stroke\n            .as_ref()\n            .map(|stroke| stroke.width_mm)\n            .unwrap_or(0.25),\n        fill_applicable,\n        fill_enabled: fill.is_some(),\n        fill_color: color_to_hex(displayed_fill_color),\n        fill_gradient_enabled,\n        fill_gradient_end_color,\n        fill_gradient_axis,\n        text_color_applicable,\n        text_color: color_to_hex(text_color.unwrap_or_else(default_black)),\n    }\n}\n""",
)

replace_once(
    TAURI,
    """fn default_fill() -> FillStyle {\n    FillStyle {\n        color: Color::Rgba {\n            r: 255,\n            g: 255,\n            b: 255,\n            a: 255,\n        },\n        gradient: None,\n    }\n}\n\nfn color_to_hex(color: Color) -> String {\n""",
    """fn default_fill() -> FillStyle {\n    FillStyle {\n        color: Color::Rgba {\n            r: 255,\n            g: 255,\n            b: 255,\n            a: 255,\n        },\n        gradient: None,\n    }\n}\n\nfn default_linear_gradient(start_color: Color) -> LinearGradient {\n    LinearGradient {\n        end_color: start_color,\n        axis: GradientAxis::AlongX,\n    }\n}\n\nfn color_to_hex(color: Color) -> String {\n""",
)

replace_once(
    APP,
    """} from './editor-interaction/connector-style-actions.mjs';\n\nconst invoke = window.__TAURI__?.core?.invoke;\n""",
    """} from './editor-interaction/connector-style-actions.mjs';\nimport {\n  appearanceControlState,\n  buildAppearanceRequest,\n} from './editor-interaction/appearance-actions.mjs';\n\nconst invoke = window.__TAURI__?.core?.invoke;\n""",
)

replace_once(
    APP,
    """  appearanceFillSection: document.querySelector('#appearance-fill-section'),\n  appearanceFillEnabled: document.querySelector('#appearance-fill-enabled'),\n  appearanceFillColor: document.querySelector('#appearance-fill-color'),\n  appearanceTextColorField: document.querySelector('#appearance-text-color-field'),\n""",
    """  appearanceFillSection: document.querySelector('#appearance-fill-section'),\n  appearanceFillEnabled: document.querySelector('#appearance-fill-enabled'),\n  appearanceFillColor: document.querySelector('#appearance-fill-color'),\n  appearanceFillGradientEnabled: document.querySelector('#appearance-fill-gradient-enabled'),\n  appearanceFillGradientControls: document.querySelector('#appearance-fill-gradient-controls'),\n  appearanceFillGradientEndColor: document.querySelector('#appearance-fill-gradient-end-color'),\n  appearanceFillGradientAxis: document.querySelector('#appearance-fill-gradient-axis'),\n  appearanceTextColorField: document.querySelector('#appearance-text-color-field'),\n""",
)

replace_once(
    APP,
    """  elements.appearanceFillEnabled.checked = appearance.fillEnabled;\n  elements.appearanceFillColor.value = appearance.fillColor;\n  elements.appearanceTextColor.value = appearance.textColor;\n""",
    """  elements.appearanceFillEnabled.checked = appearance.fillEnabled;\n  elements.appearanceFillColor.value = appearance.fillColor;\n  elements.appearanceFillGradientEnabled.checked = appearance.fillGradientEnabled;\n  elements.appearanceFillGradientEndColor.value = appearance.fillGradientEndColor;\n  elements.appearanceFillGradientAxis.value = appearance.fillGradientAxis;\n  elements.appearanceTextColor.value = appearance.textColor;\n""",
)

replace_once(
    APP,
    """function updateAppearanceEnabledState() {\n  elements.appearanceStrokeColor.disabled = !elements.appearanceStrokeEnabled.checked;\n  elements.appearanceStrokeWidth.disabled = !elements.appearanceStrokeEnabled.checked;\n  elements.appearanceFillColor.disabled = !elements.appearanceFillEnabled.checked;\n}\n""",
    """function updateAppearanceEnabledState() {\n  elements.appearanceStrokeColor.disabled = !elements.appearanceStrokeEnabled.checked;\n  elements.appearanceStrokeWidth.disabled = !elements.appearanceStrokeEnabled.checked;\n  const fillState = appearanceControlState({\n    fillEnabled: elements.appearanceFillEnabled.checked,\n    fillGradientEnabled: elements.appearanceFillGradientEnabled.checked,\n  });\n  elements.appearanceFillColor.disabled = fillState.fillColorDisabled;\n  elements.appearanceFillGradientEnabled.disabled = fillState.gradientToggleDisabled;\n  elements.appearanceFillGradientControls.hidden = fillState.gradientDetailsDisabled;\n  elements.appearanceFillGradientEndColor.disabled = fillState.gradientDetailsDisabled;\n  elements.appearanceFillGradientAxis.disabled = fillState.gradientDetailsDisabled;\n}\n""",
)

replace_once(
    APP,
    """async function applyAppearance(event) {\n  event.preventDefault();\n  const primary = currentSelectionProperties?.primary;\n  const baseline = appearanceBaseline;\n  if (!invoke || !primary || !baseline) {\n    return;\n  }\n  const request = { elementId: primary.elementId };\n  if (baseline.strokeApplicable) {\n    if (elements.appearanceStrokeEnabled.checked !== baseline.strokeEnabled) {\n      request.strokeEnabled = elements.appearanceStrokeEnabled.checked;\n    }\n    if (elements.appearanceStrokeColor.value.toLowerCase() !== baseline.strokeColor.toLowerCase()) {\n      request.strokeColor = elements.appearanceStrokeColor.value;\n    }\n    const width = Number(elements.appearanceStrokeWidth.value);\n    if (!Number.isFinite(width) || width <= 0) {\n      setStatus('Stroke width must be a finite positive value');\n      return;\n    }\n    if (width !== baseline.strokeWidthMm) {\n      request.strokeWidthMm = width;\n    }\n  }\n  if (baseline.fillApplicable) {\n    if (elements.appearanceFillEnabled.checked !== baseline.fillEnabled) {\n      request.fillEnabled = elements.appearanceFillEnabled.checked;\n    }\n    if (elements.appearanceFillColor.value.toLowerCase() !== baseline.fillColor.toLowerCase()) {\n      request.fillColor = elements.appearanceFillColor.value;\n    }\n  }\n  if (\n    baseline.textColorApplicable &&\n    elements.appearanceTextColor.value.toLowerCase() !== baseline.textColor.toLowerCase()\n  ) {\n    request.textColor = elements.appearanceTextColor.value;\n  }\n  if (Object.keys(request).length === 1) {\n    setStatus('Appearance unchanged');\n    return;\n  }\n\n  setBusy(true);\n""",
    """async function applyAppearance(event) {\n  event.preventDefault();\n  const primary = currentSelectionProperties?.primary;\n  const baseline = appearanceBaseline;\n  if (!invoke || !primary || !baseline) {\n    return;\n  }\n\n  let request;\n  try {\n    request = buildAppearanceRequest({\n      elementId: primary.elementId,\n      baseline,\n      strokeEnabled: elements.appearanceStrokeEnabled.checked,\n      strokeColor: elements.appearanceStrokeColor.value,\n      strokeWidthMm: elements.appearanceStrokeWidth.value,\n      fillEnabled: elements.appearanceFillEnabled.checked,\n      fillColor: elements.appearanceFillColor.value,\n      fillGradientEnabled: elements.appearanceFillGradientEnabled.checked,\n      fillGradientEndColor: elements.appearanceFillGradientEndColor.value,\n      fillGradientAxis: elements.appearanceFillGradientAxis.value,\n      textColor: elements.appearanceTextColor.value,\n    });\n  } catch (error) {\n    setStatus(String(error?.message ?? error));\n    return;\n  }\n  if (!request) {\n    setStatus('Appearance unchanged');\n    return;\n  }\n\n  setBusy(true);\n""",
)

replace_once(
    APP,
    """elements.appearanceStrokeEnabled.addEventListener('change', updateAppearanceEnabledState);\nelements.appearanceFillEnabled.addEventListener('change', updateAppearanceEnabledState);\n""",
    """elements.appearanceStrokeEnabled.addEventListener('change', updateAppearanceEnabledState);\nelements.appearanceFillEnabled.addEventListener('change', updateAppearanceEnabledState);\nelements.appearanceFillGradientEnabled.addEventListener('change', updateAppearanceEnabledState);\n""",
)

replace_once(
    HTML,
    """              <div class=\"appearance-heading\">\n                <h3>Appearance</h3>\n                <span>basic</span>\n              </div>\n""",
    """              <div class=\"appearance-heading\">\n                <h3>Appearance</h3>\n                <span>stroke · fill · gradient</span>\n              </div>\n""",
)

replace_once(
    HTML,
    """              <div id=\"appearance-fill-section\" class=\"appearance-section\">\n                <label class=\"appearance-toggle\"><input id=\"appearance-fill-enabled\" type=\"checkbox\" /> Fill</label>\n                <label class=\"appearance-color-field\">Color <input id=\"appearance-fill-color\" type=\"color\" value=\"#ffffff\" /></label>\n              </div>\n""",
    """              <div id=\"appearance-fill-section\" class=\"appearance-section\">\n                <label class=\"appearance-toggle\"><input id=\"appearance-fill-enabled\" type=\"checkbox\" /> Fill</label>\n                <label class=\"appearance-color-field\">Start color <input id=\"appearance-fill-color\" type=\"color\" value=\"#ffffff\" /></label>\n                <label class=\"appearance-toggle\"><input id=\"appearance-fill-gradient-enabled\" type=\"checkbox\" /> Linear gradient</label>\n                <div id=\"appearance-fill-gradient-controls\" class=\"appearance-section\" hidden>\n                  <label class=\"appearance-color-field\">End color <input id=\"appearance-fill-gradient-end-color\" type=\"color\" value=\"#ffffff\" /></label>\n                  <label class=\"property-field\">Direction\n                    <select id=\"appearance-fill-gradient-axis\">\n                      <option value=\"along_x\">Horizontal</option>\n                      <option value=\"along_y\">Vertical</option>\n                    </select>\n                  </label>\n                </div>\n                <p class=\"property-note\">Imported system colours remain unchanged until you edit the corresponding colour picker.</p>\n              </div>\n""",
)

replace_once(
    DOC,
    """- connector start/end marker and line-style editing for straight and orthogonal connectors, including preservation of imported custom legacy style codes until explicitly replaced.\n""",
    """- connector start/end marker and line-style editing for straight and orthogonal connectors, including preservation of imported custom legacy style codes until explicitly replaced;\n- linear shape fill gradients with independent start/end colours and horizontal/vertical direction, using the existing one-step appearance history and DDNX style model.\n""",
)

replace_once(
    DOC,
    """- Basic appearance covers shape stroke/fill, text colour, and standard connector markers/line styles. Gradients, advanced text formatting, and direct editing of arbitrary custom legacy connector style codes remain outside the current controls.\n""",
    """- Basic appearance covers shape stroke/fill including linear gradients, text colour, and standard connector markers/line styles. Advanced text formatting and direct editing of arbitrary custom legacy connector style codes remain outside the current controls.\n""",
)

print("Fill-gradient integration anchors applied successfully.")
