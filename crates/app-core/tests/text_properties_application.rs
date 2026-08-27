use app_core::ApplicationSession;
use ddnx::PackageLimits;
use next_domain::{
    AnchorSet, Color, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,
    ElementId, ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, RichTextDocument,
    RichTextToken, Scene, ScriptPosition, Size, TextBlock, TextHorizontalAlignment, TextLayout,
    TextStyle, TextTailDirective, TextTailKind, TextVerticalAlignment,
};

fn bounds() -> Rect {
    Rect {
        x: 20.0,
        y: 30.0,
        width: 70.0,
        height: 24.0,
    }
}

fn layout() -> TextLayout {
    TextLayout {
        horizontal: TextHorizontalAlignment::Left,
        vertical: TextVerticalAlignment::Top,
        margin_mm: 1.5,
    }
}

fn text_block(text: &str, style: TextStyle) -> TextBlock {
    TextBlock {
        content: RichTextDocument {
            tokens: vec![RichTextToken::Text {
                text: text.to_owned(),
                style,
            }],
            tail: None,
            diagnostics: Vec::new(),
        },
        layout: layout(),
    }
}

fn initial_style() -> TextStyle {
    TextStyle {
        bold: false,
        italic: false,
        underline: false,
        strikeout: true,
        script: ScriptPosition::Superscript,
        overline: true,
        symbol_font: false,
        font_family: None,
        font_size_pt: None,
        color: Some(Color::SystemPalette { index: 9 }),
    }
}

fn updated_style() -> TextStyle {
    TextStyle {
        bold: true,
        italic: true,
        underline: true,
        strikeout: true,
        script: ScriptPosition::Superscript,
        overline: true,
        symbol_font: false,
        font_family: Some("Inter".to_owned()),
        font_size_pt: Some(14),
        color: Some(Color::SystemPalette { index: 9 }),
    }
}

fn fixture() -> (NextArtifact, ElementId, TextBlock) {
    let element_id = ElementId::new();
    let initial = text_block("Alpha", initial_style());
    let element = Element {
        id: element_id,
        name: "Text".to_owned(),
        bounds_mm: bounds(),
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: Some(initial.clone()),
        kind: ElementKind::Text,
        import: None,
    };
    let document = Document {
        id: DocumentId::new(),
        name: "Text formatting application test".to_owned(),
        defaults: DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Transparent,
        },
        master_layers: Vec::new(),
        pages: vec![Page {
            id: PageId::new(),
            name: "Page 1".to_owned(),
            size_mm: Size {
                width: 210.0,
                height: 297.0,
            },
            layers: vec![Layer {
                id: LayerId::new(),
                name: "Layer 1".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: vec![element_id],
                    elements: vec![element],
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (NextArtifact::document(document), element_id, initial)
}

fn block(app: &ApplicationSession, element_id: ElementId) -> &TextBlock {
    app.session().document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == element_id)
        .unwrap()
        .text
        .as_ref()
        .unwrap()
}

#[test]
fn text_content_and_uniform_style_are_one_history_step_and_survive_ddnx() {
    let (artifact, element_id, initial) = fixture();
    let mut app = ApplicationSession::from_artifact(artifact).unwrap();
    let initial_history = app.session().current_history_state();
    let mut updated = text_block("Beta", updated_style());
    updated.layout = TextLayout {
        horizontal: TextHorizontalAlignment::Center,
        vertical: TextVerticalAlignment::Bottom,
        margin_mm: 3.25,
    };

    assert!(
        app.commit_element_properties(element_id, bounds(), 0.0, Some(Some(updated.clone())))
            .unwrap()
    );
    let updated_history = app.session().current_history_state();
    assert_ne!(updated_history, initial_history);
    assert_eq!(block(&app, element_id), &updated);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened =
        ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(block(&reopened, element_id), &updated);

    assert!(app.undo().unwrap());
    assert_eq!(app.session().current_history_state(), initial_history);
    assert_eq!(block(&app, element_id), &initial);

    assert!(app.redo().unwrap());
    assert_eq!(app.session().current_history_state(), updated_history);
    assert_eq!(block(&app, element_id), &updated);

    let before_noop = app.session().current_history_state();
    assert!(
        !app.commit_element_properties(element_id, bounds(), 0.0, Some(Some(updated)))
            .unwrap()
    );
    assert_eq!(app.session().current_history_state(), before_noop);
}

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
        app.commit_element_properties(element_id, bounds(), 0.0, Some(Some(protected.clone())),)
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
        app.commit_element_properties(element_id, bounds(), 0.0, Some(Some(layout_only.clone())),)
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
