from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, found {count}: {old[:90]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


LIB = "apps/desktop/src-tauri/src/lib.rs"

replace_once(
    LIB,
    "    Port, PortId, Rect, RichTextDocument, RichTextToken, Scene, ScriptPosition, Size, StrokeStyle,\n    StyleId, TextBlock, TextHorizontalAlignment, TextLayout, TextStyle, TextVerticalAlignment,\n",
    "    Port, PortId, Rect, RichTextDocument, RichTextToken, Scene, Size, StrokeStyle, StyleId,\n    TextBlock, TextHorizontalAlignment, TextLayout, TextStyle, TextVerticalAlignment,\n",
)

replace_once(
    LIB,
    '''struct TextStyleDto {\n    bold: bool,\n    italic: bool,\n    underline: bool,\n    strikeout: bool,\n    script: ScriptPosition,\n    overline: bool,\n    symbol_font: bool,\n    font_family: Option<String>,\n    font_size_pt: Option<u16>,\n    color: Option<Color>,\n}\n''',
    '''struct TextStyleDto {\n    bold: bool,\n    italic: bool,\n    underline: bool,\n    font_family: Option<String>,\n    font_size_pt: Option<u16>,\n}\n''',
)

replace_once(
    LIB,
    '''        let next_style = match text_style {\n            Some(style) => text_style_from_dto(style)?,\n            None => common_style.unwrap_or_default(),\n        };\n''',
    '''        let baseline_style = common_style.unwrap_or_default();\n        let next_style = match text_style {\n            Some(style) => text_style_from_dto(baseline_style, style)?,\n            None => baseline_style,\n        };\n''',
)

replace_once(
    LIB,
    '''fn text_style_dto(style: TextStyle) -> TextStyleDto {\n    TextStyleDto {\n        bold: style.bold,\n        italic: style.italic,\n        underline: style.underline,\n        strikeout: style.strikeout,\n        script: style.script,\n        overline: style.overline,\n        symbol_font: style.symbol_font,\n        font_family: style.font_family,\n        font_size_pt: style.font_size_pt,\n        color: style.color,\n    }\n}\n\nfn text_style_from_dto(style: TextStyleDto) -> Result<TextStyle, CommandError> {\n    if style.font_size_pt == Some(0) {\n        return Err(CommandError::new(\n            "invalid_text_font_size",\n            "Text font size must be a positive whole number of points.",\n        ));\n    }\n    let font_family = style.font_family.and_then(|family| {\n        let trimmed = family.trim();\n        (!trimmed.is_empty()).then(|| trimmed.to_owned())\n    });\n    Ok(TextStyle {\n        bold: style.bold,\n        italic: style.italic,\n        underline: style.underline,\n        strikeout: style.strikeout,\n        script: style.script,\n        overline: style.overline,\n        symbol_font: style.symbol_font,\n        font_family,\n        font_size_pt: style.font_size_pt,\n        color: style.color,\n    })\n}\n''',
    '''fn text_style_dto(style: TextStyle) -> TextStyleDto {\n    TextStyleDto {\n        bold: style.bold,\n        italic: style.italic,\n        underline: style.underline,\n        font_family: style.font_family,\n        font_size_pt: style.font_size_pt,\n    }\n}\n\nfn text_style_from_dto(\n    mut baseline: TextStyle,\n    style: TextStyleDto,\n) -> Result<TextStyle, CommandError> {\n    if style.font_size_pt == Some(0) {\n        return Err(CommandError::new(\n            "invalid_text_font_size",\n            "Text font size must be a positive whole number of points.",\n        ));\n    }\n    baseline.font_family = style.font_family.and_then(|family| {\n        let trimmed = family.trim();\n        (!trimmed.is_empty()).then(|| trimmed.to_owned())\n    });\n    baseline.font_size_pt = style.font_size_pt;\n    baseline.bold = style.bold;\n    baseline.italic = style.italic;\n    baseline.underline = style.underline;\n    Ok(baseline)\n}\n''',
)

print("restricted uniform text formatting IPC applied")
