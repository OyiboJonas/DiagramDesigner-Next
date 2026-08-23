use serde::Serialize;

use crate::{
    LegacyDecoded,
    encoding::{
        DecodedLegacyString, EncodingDecision, LegacyEncoding, decode_ansi_string,
        decode_with_encoding,
    },
    object::{LegacyBaseObject, LegacyCurveBase, LegacyObject, LegacyTextPayload},
    text_markup::{RichTextDocument, RichTextToken, TailDirectiveKind, parse_legacy_text_markup},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRole {
    DefaultFontName,
    PageName,
    ObjectName,
    ObjectText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NormalizedTextEntry {
    pub path: String,
    pub role: TextRole,
    /// Preserved source bytes remain available even after successful decoding.
    pub raw: Vec<u8>,
    pub decoded: DecodedLegacyString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich_text: Option<RichTextDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextNormalizationSummary {
    pub fallback_encoding: LegacyEncoding,
    pub entries: usize,
    pub object_text_entries: usize,
    pub decode_error_entries: usize,
    pub markup_diagnostics: usize,
    pub symbol_glyphs: usize,
    pub action_tails: usize,
    pub hint_tails: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextNormalizationReport {
    pub summary: TextNormalizationSummary,
    pub entries: Vec<NormalizedTextEntry>,
}

fn object_base(object: &LegacyObject) -> &LegacyBaseObject {
    match object {
        LegacyObject::Text { payload } => &payload.base,
        LegacyObject::Rectangle { shape, .. }
        | LegacyObject::Ellipse { shape }
        | LegacyObject::Polygon { shape, .. }
        | LegacyObject::Flowchart { shape, .. } => &shape.line.text.base,
        LegacyObject::StraightLine { connector }
        | LegacyObject::ConnectorLine { connector, .. } => &connector.line.text.base,
        LegacyObject::Bitmap { picture, .. }
        | LegacyObject::Metafile { picture, .. }
        | LegacyObject::InheritedLayer { picture, .. } => &picture.base,
        LegacyObject::Group { base, .. } => base,
        LegacyObject::CurveLine { base, .. } => match base {
            LegacyCurveBase::Line { line } => &line.text.base,
            LegacyCurveBase::Connector { connector } => &connector.line.text.base,
        },
    }
}

fn object_text(object: &LegacyObject) -> Option<&LegacyTextPayload> {
    match object {
        LegacyObject::Text { payload } => Some(payload),
        LegacyObject::Rectangle { shape, .. }
        | LegacyObject::Ellipse { shape }
        | LegacyObject::Polygon { shape, .. }
        | LegacyObject::Flowchart { shape, .. } => Some(&shape.line.text),
        LegacyObject::StraightLine { connector }
        | LegacyObject::ConnectorLine { connector, .. } => Some(&connector.line.text),
        LegacyObject::CurveLine { base, .. } => match base {
            LegacyCurveBase::Line { line } => Some(&line.text),
            LegacyCurveBase::Connector { connector } => Some(&connector.line.text),
        },
        LegacyObject::Bitmap { .. }
        | LegacyObject::Metafile { .. }
        | LegacyObject::Group { .. }
        | LegacyObject::InheritedLayer { .. } => None,
    }
}

fn push_entry(
    entries: &mut Vec<NormalizedTextEntry>,
    path: String,
    role: TextRole,
    raw: &[u8],
    decoded: DecodedLegacyString,
    rich_text: bool,
) {
    if raw.is_empty() {
        return;
    }

    let rich_text = rich_text.then(|| parse_legacy_text_markup(&decoded.text));
    entries.push(NormalizedTextEntry {
        path,
        role,
        raw: raw.to_vec(),
        decoded,
        rich_text,
    });
}

fn normalize_object_list(
    objects: &[LegacyObject],
    list_path: &str,
    font_charset: Option<u8>,
    fallback: LegacyEncoding,
    entries: &mut Vec<NormalizedTextEntry>,
) {
    for (index, object) in objects.iter().enumerate() {
        let object_path = format!("{list_path}/object/{index}");
        let base = object_base(object);
        let decode = |raw: &[u8]| match font_charset {
            Some(charset) => decode_ansi_string(raw, charset, fallback),
            None => decode_with_encoding(raw, fallback, EncodingDecision::ExplicitOverride),
        };

        push_entry(
            entries,
            format!("{object_path}/name"),
            TextRole::ObjectName,
            &base.name_raw,
            decode(&base.name_raw),
            false,
        );

        if let Some(text) = object_text(object) {
            push_entry(
                entries,
                format!("{object_path}/text"),
                TextRole::ObjectText,
                &text.text_raw,
                decode(&text.text_raw),
                true,
            );
        }

        if let LegacyObject::Group { children, .. } = object {
            normalize_object_list(
                children,
                &format!("{object_path}/group"),
                font_charset,
                fallback,
                entries,
            );
        }
    }
}

/// Normalize all textual fields in a decoded legacy document while preserving
/// original bytes and recording the exact charset decision for each entry.
///
/// DDD uses the stored `DefaultFontCharSet` where it identifies a concrete code
/// page; `DEFAULT_CHARSET`, Symbol/OEM and unknown values fall back explicitly.
/// DDT contains no top-level charset, so the caller-selected fallback is marked
/// as an explicit override.
pub fn normalize_document_text(
    document: &LegacyDecoded,
    fallback: LegacyEncoding,
) -> TextNormalizationReport {
    let mut entries = Vec::new();

    match document {
        LegacyDecoded::Ddd(container) => {
            let charset = container.defaults.default_font_charset;
            push_entry(
                &mut entries,
                "document/default_font_name".to_owned(),
                TextRole::DefaultFontName,
                &container.defaults.default_font_name_raw,
                decode_ansi_string(&container.defaults.default_font_name_raw, charset, fallback),
                false,
            );

            for (page_index, page) in container.pages.iter().enumerate() {
                push_entry(
                    &mut entries,
                    format!("page/{page_index}/name"),
                    TextRole::PageName,
                    &page.name_raw,
                    decode_ansi_string(&page.name_raw, charset, fallback),
                    false,
                );
                for (layer_index, layer) in page.layers.iter().enumerate() {
                    normalize_object_list(
                        &layer.objects,
                        &format!("page/{page_index}/layer/{layer_index}"),
                        Some(charset),
                        fallback,
                        &mut entries,
                    );
                }
            }

            if let Some(stencil) = &container.stencil {
                normalize_object_list(
                    &stencil.objects,
                    "stencil",
                    Some(charset),
                    fallback,
                    &mut entries,
                );
            }
        }
        LegacyDecoded::Ddt(template) => {
            normalize_object_list(&template.objects, "template", None, fallback, &mut entries);
        }
    }

    let object_text_entries = entries
        .iter()
        .filter(|entry| entry.role == TextRole::ObjectText)
        .count();
    let decode_error_entries = entries
        .iter()
        .filter(|entry| entry.decoded.had_errors)
        .count();
    let markup_diagnostics = entries
        .iter()
        .filter_map(|entry| entry.rich_text.as_ref())
        .map(|rich_text| rich_text.diagnostics.len())
        .sum();
    let symbol_glyphs = entries
        .iter()
        .filter_map(|entry| entry.rich_text.as_ref())
        .flat_map(|rich_text| &rich_text.tokens)
        .filter(|token| matches!(token, RichTextToken::SymbolGlyph { .. }))
        .count();
    let action_tails = entries
        .iter()
        .filter_map(|entry| entry.rich_text.as_ref())
        .filter_map(|rich_text| rich_text.tail.as_ref())
        .filter(|tail| tail.kind == TailDirectiveKind::Action)
        .count();
    let hint_tails = entries
        .iter()
        .filter_map(|entry| entry.rich_text.as_ref())
        .filter_map(|rich_text| rich_text.tail.as_ref())
        .filter(|tail| tail.kind == TailDirectiveKind::Hint)
        .count();

    TextNormalizationReport {
        summary: TextNormalizationSummary {
            fallback_encoding: fallback,
            entries: entries.len(),
            object_text_entries,
            decode_error_entries,
            markup_diagnostics,
            symbol_glyphs,
            action_tails,
            hint_tails,
        },
        entries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        LegacyDecoded,
        container::{LegacyContainer, LegacyContainerDefaults, LegacyLayer, LegacyPage},
        object::{LegacyBaseObject, LegacyRect},
    };

    #[test]
    fn normalizes_cp1252_names_and_markup_while_preserving_raw_bytes() {
        let raw_name = vec![b'G', b'r', 0xfc, b'p', b'p', b'e'];
        let raw_text = b"Status: \\BOK\\b".to_vec();
        let text_object = LegacyObject::Text {
            payload: LegacyTextPayload {
                base: LegacyBaseObject {
                    name_raw: raw_name.clone(),
                    position: LegacyRect {
                        left: 0,
                        top: 0,
                        right: 10,
                        bottom: 10,
                    },
                    anchors: 0,
                },
                text_raw: raw_text.clone(),
                text_x_align: 0,
                text_y_align: 0,
                text_color: 0,
                margin: 0,
                angle: 0.0,
            },
        };
        let document = LegacyDecoded::Ddd(LegacyContainer {
            defaults: LegacyContainerDefaults {
                default_font_name_raw: b"Arial".to_vec(),
                default_font_size: 10,
                default_font_style: 0,
                default_font_charset: 1,
                object_shadows: false,
                auto_line_break: true,
                connector_label_style: 1,
            },
            pages: vec![LegacyPage {
                width: 100,
                height: 100,
                name_raw: vec![b'S', 0xe4, b'i', b't', b'e'],
                layers: vec![LegacyLayer {
                    draw_color: -1,
                    objects: vec![text_object],
                }],
            }],
            stencil: None,
            trailing_bytes: 0,
        });

        let report = normalize_document_text(&document, LegacyEncoding::Windows1252);
        assert_eq!(report.summary.object_text_entries, 1);
        assert_eq!(report.summary.decode_error_entries, 0);
        assert_eq!(report.summary.markup_diagnostics, 0);
        assert_eq!(report.summary.symbol_glyphs, 0);
        assert_eq!(report.summary.action_tails, 0);
        assert_eq!(report.summary.hint_tails, 0);
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.decoded.text == "Grüppe" && entry.raw == raw_name)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.decoded.text == "Säite")
        );
        let text_entry = report
            .entries
            .iter()
            .find(|entry| entry.role == TextRole::ObjectText)
            .unwrap();
        assert_eq!(text_entry.raw, raw_text);
        assert!(text_entry.rich_text.is_some());
    }

    #[test]
    fn summarizes_unresolved_symbol_glyphs_and_inert_tails() {
        let document = LegacyDecoded::Ddt(crate::template::LegacyTemplate {
            width: 1,
            height: 1,
            objects: vec![LegacyObject::Text {
                payload: LegacyTextPayload {
                    base: LegacyBaseObject {
                        name_raw: b"Symbol text".to_vec(),
                        position: LegacyRect {
                            left: 0,
                            top: 0,
                            right: 1,
                            bottom: 1,
                        },
                        anchors: 0,
                    },
                    text_raw: b"\\SAB\\s\\#\\A2".to_vec(),
                    text_x_align: 0,
                    text_y_align: 0,
                    text_color: 0,
                    margin: 0,
                    angle: 0.0,
                },
            }],
            trailing_bytes: 0,
        });

        let report = normalize_document_text(&document, LegacyEncoding::Windows1252);
        assert_eq!(report.summary.symbol_glyphs, 2);
        assert_eq!(report.summary.action_tails, 1);
        assert_eq!(report.summary.hint_tails, 0);
        assert_eq!(report.summary.markup_diagnostics, 0);
    }

    #[test]
    fn ddt_marks_fallback_as_explicit_override() {
        let document = LegacyDecoded::Ddt(crate::template::LegacyTemplate {
            width: 1,
            height: 1,
            objects: vec![LegacyObject::Text {
                payload: LegacyTextPayload {
                    base: LegacyBaseObject {
                        name_raw: b"Text".to_vec(),
                        position: LegacyRect {
                            left: 0,
                            top: 0,
                            right: 1,
                            bottom: 1,
                        },
                        anchors: 0,
                    },
                    text_raw: vec![0xe4],
                    text_x_align: 0,
                    text_y_align: 0,
                    text_color: 0,
                    margin: 0,
                    angle: 0.0,
                },
            }],
            trailing_bytes: 0,
        });
        let report = normalize_document_text(&document, LegacyEncoding::Windows1252);
        let text = report
            .entries
            .iter()
            .find(|entry| entry.role == TextRole::ObjectText)
            .unwrap();
        assert_eq!(text.decoded.text, "ä");
        assert_eq!(text.decoded.decision, EncodingDecision::ExplicitOverride);
    }
}
