use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScriptPosition {
    #[default]
    Normal,
    Subscript,
    Superscript,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct RichTextStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub script: ScriptPosition,
    pub overline: bool,
    pub symbol_font: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size_pt: Option<u16>,
    /// Legacy `\Crrggbb` is represented in ordinary RGB order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_rgb: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextToken {
    Text {
        text: String,
        style: RichTextStyle,
    },
    NewLine,
    PageNumber {
        style: RichTextStyle,
    },
    PageCount {
        style: RichTextStyle,
    },
    PageName {
        style: RichTextStyle,
    },
    /// A glyph rendered by the legacy Windows Symbol font whose portable
    /// Unicode meaning has not been proven at this migration boundary.
    SymbolGlyph {
        legacy_glyph: char,
        style: RichTextStyle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TailDirectiveKind {
    Action,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TailDirective {
    pub kind: TailDirectiveKind,
    /// Decoded tail after `\A` / `\N`. It is deliberately not interpreted as a
    /// path/URL here; the migration/security layer decides how to classify it.
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MarkupDiagnostic {
    pub char_offset: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct RichTextDocument {
    pub tokens: Vec<RichTextToken>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail: Option<TailDirective>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<MarkupDiagnostic>,
}

fn push_text(tokens: &mut Vec<RichTextToken>, text: &str, style: &RichTextStyle) {
    if text.is_empty() {
        return;
    }

    // `\S` in the Delphi renderer switches the active font to Windows Symbol.
    // Ordinary characters following it are glyph codes, not Unicode semantics.
    // Keep them explicit until a font-aware mapping is proven.
    if style.symbol_font {
        for legacy_glyph in text.chars() {
            tokens.push(RichTextToken::SymbolGlyph {
                legacy_glyph,
                style: style.clone(),
            });
        }
        return;
    }

    if let Some(RichTextToken::Text {
        text: previous,
        style: previous_style,
    }) = tokens.last_mut()
    {
        if previous_style == style {
            previous.push_str(text);
            return;
        }
    }

    tokens.push(RichTextToken::Text {
        text: text.to_owned(),
        style: style.clone(),
    });
}

fn push_char(tokens: &mut Vec<RichTextToken>, value: char, style: &RichTextStyle) {
    let mut buffer = [0; 4];
    push_text(tokens, value.encode_utf8(&mut buffer), style);
}

fn push_portable_symbol(tokens: &mut Vec<RichTextToken>, value: char, style: &RichTextStyle) {
    // Source-defined `WriteSymbol(...)` escapes below have an unambiguous
    // Adobe Symbol Encoding mapping. Once mapped to Unicode, the token no
    // longer carries Symbol-font rendering semantics.
    let mut portable_style = style.clone();
    portable_style.symbol_font = false;
    push_char(tokens, value, &portable_style);
}

fn parse_hex_rgb(chars: &[char], start: usize) -> Option<u32> {
    if start + 6 > chars.len() {
        return None;
    }
    let mut value = 0_u32;
    for ch in &chars[start..start + 6] {
        let digit = ch.to_digit(16)?;
        value = (value << 4) | digit;
    }
    Some(value)
}

/// Parse the legacy `TTextObject.Draw.ParseText` command language into a
/// renderer-independent token stream.
///
/// Free Symbol-font runs and action/hint tails are preserved rather than guessed
/// or executed. The eight explicit `WriteSymbol(...)` source escapes are mapped
/// to Unicode only because their Adobe Symbol Encoding semantics are unambiguous.
pub fn parse_legacy_text_markup(input: &str) -> RichTextDocument {
    let chars: Vec<char> = input.chars().collect();
    let mut document = RichTextDocument::default();
    let mut style = RichTextStyle::default();
    let mut index = 0;
    let mut plain_start = 0;

    while index < chars.len() {
        if chars[index] != '\\' {
            index += 1;
            continue;
        }

        if plain_start < index {
            let text: String = chars[plain_start..index].iter().collect();
            push_text(&mut document.tokens, &text, &style);
        }

        let command_offset = index;
        index += 1;
        if index >= chars.len() {
            push_char(&mut document.tokens, '\\', &style);
            document.diagnostics.push(MarkupDiagnostic {
                char_offset: command_offset,
                message: "trailing backslash preserved literally".to_owned(),
            });
            plain_start = index;
            break;
        }

        let command = chars[index];
        match command {
            'A' | 'N' => {
                let value: String = chars[index + 1..].iter().collect();
                document.tail = Some(TailDirective {
                    kind: if command == 'A' {
                        TailDirectiveKind::Action
                    } else {
                        TailDirectiveKind::Hint
                    },
                    value,
                });
                index = chars.len();
                plain_start = index;
                break;
            }
            'I' => style.italic = true,
            'i' => style.italic = false,
            'B' => style.bold = true,
            'b' => style.bold = false,
            'U' => style.underline = true,
            'u' => style.underline = false,
            'T' => style.strikeout = true,
            't' => style.strikeout = false,
            'L' => style.script = ScriptPosition::Subscript,
            'H' => style.script = ScriptPosition::Superscript,
            'l' | 'h' => style.script = ScriptPosition::Normal,
            'O' => {
                if !style.overline {
                    style.overline = true;
                }
            }
            'o' => {
                if style.overline {
                    style.overline = false;
                } else {
                    // The Delphi 7 source contains byte 0x95 here, which is the
                    // Windows-1252 bullet in the validated build/corpus context.
                    push_char(&mut document.tokens, '•', &style);
                }
            }
            'S' => style.symbol_font = true,
            's' => style.symbol_font = false,
            'n' => document.tokens.push(RichTextToken::NewLine),
            'p' => document.tokens.push(RichTextToken::PageNumber {
                style: style.clone(),
            }),
            'c' => document.tokens.push(RichTextToken::PageCount {
                style: style.clone(),
            }),
            'P' => document.tokens.push(RichTextToken::PageName {
                style: style.clone(),
            }),
            '0'..='9' => {
                let start = index;
                let mut end = index + 1;
                while end < chars.len() && end - start < 3 && chars[end].is_ascii_digit() {
                    end += 1;
                }
                let digits: String = chars[start..end].iter().collect();
                style.font_size_pt = digits.parse::<u16>().ok();
                index = end - 1;
            }
            'C' => {
                if let Some(rgb) = parse_hex_rgb(&chars, index + 1) {
                    style.color_rgb = Some(rgb);
                    index += 6;
                } else {
                    document.diagnostics.push(MarkupDiagnostic {
                        char_offset: command_offset,
                        message: "invalid or truncated legacy text color escape".to_owned(),
                    });
                    push_text(&mut document.tokens, "\\C", &style);
                }
            }
            '"' => {
                let name_start = index + 1;
                let mut end = name_start;
                while end < chars.len() && chars[end] != '"' {
                    end += 1;
                }
                style.font_family = Some(chars[name_start..end].iter().collect());
                if end == chars.len() {
                    document.diagnostics.push(MarkupDiagnostic {
                        char_offset: command_offset,
                        message: "unterminated legacy font-name escape".to_owned(),
                    });
                    index = chars.len();
                    plain_start = index;
                    break;
                }
                index = end;
            }
            '@' => {
                let raw_start = index + 1;
                let mut end = raw_start;
                let mut close = None;
                while end + 1 < chars.len() {
                    if chars[end] == '\\' && chars[end + 1] == '@' {
                        close = Some(end);
                        break;
                    }
                    end += 1;
                }

                match close {
                    Some(close_index) => {
                        let raw: String = chars[raw_start..close_index].iter().collect();
                        push_text(&mut document.tokens, &raw, &style);
                        index = close_index + 1;
                    }
                    None => {
                        let raw: String = chars[raw_start..].iter().collect();
                        push_text(&mut document.tokens, &raw, &style);
                        index = chars.len();
                        plain_start = index;
                        break;
                    }
                }
            }
            '.' => push_char(&mut document.tokens, '·', &style),
            '+' => push_char(&mut document.tokens, '±', &style),
            '*' => push_char(&mut document.tokens, '×', &style),
            '\'' => push_char(&mut document.tokens, '°', &style),
            // `TextObject.pas` calls WriteSymbol with these eight source glyph
            // codes. In Adobe Symbol Encoding they map unambiguously to:
            // A8→♦, B9→≠, BB→≈, B3→≥, A3→≤, D6→√, B8→÷, A5→∞.
            '#' => push_portable_symbol(&mut document.tokens, '♦', &style),
            '=' => push_portable_symbol(&mut document.tokens, '≠', &style),
            '~' => push_portable_symbol(&mut document.tokens, '≈', &style),
            '>' => push_portable_symbol(&mut document.tokens, '≥', &style),
            '<' => push_portable_symbol(&mut document.tokens, '≤', &style),
            '/' => push_portable_symbol(&mut document.tokens, '√', &style),
            '-' => push_portable_symbol(&mut document.tokens, '÷', &style),
            '§' => push_portable_symbol(&mut document.tokens, '∞', &style),
            // This also reproduces `\\` -> `\`: the source's default branch
            // writes the character following the escape.
            other => push_char(&mut document.tokens, other, &style),
        }

        index += 1;
        plain_start = index;
    }

    if plain_start < chars.len() {
        let text: String = chars[plain_start..].iter().collect();
        push_text(&mut document.tokens, &text, &style);
    }

    document
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_style_toggles_and_merges_adjacent_text() {
        let doc = parse_legacy_text_markup("A\\Bbold\\b normal");
        assert_eq!(doc.tokens.len(), 3);
        assert!(matches!(
            &doc.tokens[1],
            RichTextToken::Text { text, style } if text == "bold" && style.bold
        ));
        assert!(matches!(
            &doc.tokens[2],
            RichTextToken::Text { text, style } if text == " normal" && !style.bold
        ));
    }

    #[test]
    fn parses_page_fields_color_font_and_size() {
        let doc = parse_legacy_text_markup("\\12\\Cff0000\\\"Arial\"X\\p/\\c/\\P");
        assert!(matches!(
            &doc.tokens[0],
            RichTextToken::Text { text, style }
                if text == "X"
                    && style.font_size_pt == Some(12)
                    && style.color_rgb == Some(0xff0000)
                    && style.font_family.as_deref() == Some("Arial")
        ));
        assert!(
            doc.tokens
                .iter()
                .any(|token| matches!(token, RichTextToken::PageNumber { .. }))
        );
        assert!(
            doc.tokens
                .iter()
                .any(|token| matches!(token, RichTextToken::PageCount { .. }))
        );
        assert!(
            doc.tokens
                .iter()
                .any(|token| matches!(token, RichTextToken::PageName { .. }))
        );
    }

    #[test]
    fn preserves_unformatted_segment_without_interpreting_backslashes() {
        let doc = parse_legacy_text_markup("before\\@\\Bnot bold\\@after");
        let texts: String = doc
            .tokens
            .iter()
            .filter_map(|token| match token {
                RichTextToken::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, "before\\Bnot boldafter");
    }

    #[test]
    fn extracts_action_tail_and_stops_render_tokens() {
        let doc = parse_legacy_text_markup("Open\\A2");
        assert_eq!(doc.tokens.len(), 1);
        assert_eq!(
            doc.tail,
            Some(TailDirective {
                kind: TailDirectiveKind::Action,
                value: "2".to_owned(),
            })
        );
    }

    #[test]
    fn maps_source_verified_write_symbol_escapes_to_unicode() {
        let cases = [
            ('#', '♦'),
            ('=', '≠'),
            ('~', '≈'),
            ('>', '≥'),
            ('<', '≤'),
            ('/', '√'),
            ('-', '÷'),
            ('§', '∞'),
        ];

        for (escape, expected) in cases {
            let doc = parse_legacy_text_markup(&format!("\\{escape}"));
            assert!(matches!(
                doc.tokens.as_slice(),
                [RichTextToken::Text { text, style }]
                    if text == &expected.to_string() && !style.symbol_font
            ));
        }
    }

    #[test]
    fn preserves_free_symbol_font_runs_as_legacy_glyphs() {
        let doc = parse_legacy_text_markup("\\SABC\\s normal");
        assert_eq!(doc.tokens.len(), 4);
        for (token, expected) in doc.tokens[..3].iter().zip(['A', 'B', 'C']) {
            assert!(matches!(
                token,
                RichTextToken::SymbolGlyph { legacy_glyph, style }
                    if *legacy_glyph == expected && style.symbol_font
            ));
        }
        assert!(matches!(
            &doc.tokens[3],
            RichTextToken::Text { text, style } if text == " normal" && !style.symbol_font
        ));
    }
}
