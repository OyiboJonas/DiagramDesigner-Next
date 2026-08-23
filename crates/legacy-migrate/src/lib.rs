use std::{collections::BTreeMap, fmt::Write as _};

use legacy_ddd::{
    LegacyDecoded, LegacyError, LegacyFormat, decode_document,
    encoding::LegacyEncoding,
    object::{
        LegacyBaseObject, LegacyConnectorPayload, LegacyCurveBase, LegacyFloatPoint,
        LegacyLinePayload, LegacyObject, LegacyShapePayload, LegacyTextPayload,
    },
    parse_header,
    reference::resolve_container_reference_graph,
    text_markup as legacy_text,
    text_normalization::{NormalizedTextEntry, normalize_document_text},
};
use next_domain::{
    AnchorSet, Asset, AssetId, AssetPayload, Color, Connection, Connector, ConnectorLabelStyle,
    CurveKind, Document, DocumentDefaults, DocumentId, Element, ElementId, ElementImportMetadata,
    ElementKind, ElementStyle, Endpoint, FillStyle, GradientAxis, ImportMetadata,
    LEGACY_UNITS_PER_MM, Layer, LayerId, LineStyle, LinearGradient, MarkerStyle, NextArtifact,
    NormalizedPoint, Page, PageId, Point, Port, PortId, Rect, RichTextDocument, RichTextToken,
    Scene, ScriptPosition, Size, StrokeStyle, StyleId, TemplateId, TemplatePalette, TextBlock,
    TextHorizontalAlignment, TextLayout, TextStyle, TextTailDirective, TextTailKind,
    TextVerticalAlignment, ValidationIssue,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const CL_NONE: i32 = 0x1fff_ffff;
const IMPORTER_NAME: &str = "diagramdesigner-next/legacy-migrate/0.1";

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error(transparent)]
    Legacy(#[from] LegacyError),
    #[error(
        "legacy reference graph is invalid: {invalid_object_indices} object indices and {invalid_link_indices} link indices"
    )]
    InvalidLegacyReferences {
        invalid_object_indices: usize,
        invalid_link_indices: usize,
    },
    #[error("invalid connector reference at {source_path}: target object index {object_index}")]
    InvalidConnectorObjectIndex {
        source_path: String,
        object_index: i32,
    },
    #[error(
        "invalid connector reference at {source_path}: target link index {link_index} for object {object_index}"
    )]
    InvalidConnectorLinkIndex {
        source_path: String,
        object_index: usize,
        link_index: u16,
    },
    #[error("Next-domain validation failed with {issue_count} issue(s)")]
    InvalidNextArtifact {
        issue_count: usize,
        issues: Vec<ValidationIssue>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct MigrationOptions {
    pub fallback_encoding: LegacyEncoding,
}

impl Default for MigrationOptions {
    fn default() -> Self {
        Self {
            fallback_encoding: LegacyEncoding::Windows1252,
        }
    }
}

struct MigrationContext {
    source_sha256: String,
    source_version: u16,
    source_format: LegacyFormat,
    text_entries: BTreeMap<String, NormalizedTextEntry>,
    styles: Vec<ElementStyle>,
    assets: Vec<Asset>,
    asset_ids_by_hash: BTreeMap<String, AssetId>,
    diagnostics: Vec<String>,
}

impl MigrationContext {
    fn stable_name(&self, path: &str) -> String {
        format!("{}:{path}", self.source_sha256)
    }

    fn document_id(&self) -> DocumentId {
        DocumentId::v5(import_namespace(), &self.stable_name("document"))
    }

    fn template_id(&self) -> TemplateId {
        TemplateId::v5(import_namespace(), &self.stable_name("template"))
    }

    fn page_id(&self, path: &str) -> PageId {
        PageId::v5(import_namespace(), &self.stable_name(path))
    }

    fn layer_id(&self, path: &str) -> LayerId {
        LayerId::v5(import_namespace(), &self.stable_name(path))
    }

    fn element_id(&self, path: &str) -> ElementId {
        ElementId::v5(import_namespace(), &self.stable_name(path))
    }

    fn style_id(&self, path: &str) -> StyleId {
        StyleId::v5(import_namespace(), &self.stable_name(path))
    }

    fn normalized_text(&self, path: &str) -> Option<&NormalizedTextEntry> {
        self.text_entries.get(path)
    }

    fn import_metadata(&self) -> ImportMetadata {
        ImportMetadata {
            source_format: match self.source_format {
                LegacyFormat::Ddd => "ddd",
                LegacyFormat::Ddt => "ddt",
            }
            .to_owned(),
            source_version: self.source_version,
            source_sha256: self.source_sha256.clone(),
            importer: IMPORTER_NAME.to_owned(),
            diagnostics: self.diagnostics.clone(),
        }
    }
}

pub fn migrate_bytes(
    bytes: &[u8],
    max_inflated_bytes: usize,
    options: MigrationOptions,
) -> Result<NextArtifact, MigrationError> {
    let header = parse_header(bytes)?;
    let source_sha256 = sha256_hex(bytes);
    let decoded = decode_document(bytes, max_inflated_bytes)?;
    migrate_decoded(
        &decoded,
        header.format,
        header.version,
        &source_sha256,
        options,
    )
}

pub fn migrate_decoded(
    decoded: &LegacyDecoded,
    source_format: LegacyFormat,
    source_version: u16,
    source_sha256: &str,
    options: MigrationOptions,
) -> Result<NextArtifact, MigrationError> {
    if let LegacyDecoded::Ddd(container) = decoded {
        let graph = resolve_container_reference_graph(container);
        if !graph.summary.is_clean() {
            return Err(MigrationError::InvalidLegacyReferences {
                invalid_object_indices: graph.summary.invalid_object_indices,
                invalid_link_indices: graph.summary.invalid_link_indices,
            });
        }
    }

    let text_report = normalize_document_text(decoded, options.fallback_encoding);
    let mut diagnostics = Vec::new();
    if text_report.summary.decode_error_entries > 0 {
        diagnostics.push(format!(
            "{} textual field(s) required replacement characters during charset decoding",
            text_report.summary.decode_error_entries
        ));
    }
    if text_report.summary.markup_diagnostics > 0 {
        diagnostics.push(format!(
            "{} legacy rich-text markup diagnostic(s) were preserved",
            text_report.summary.markup_diagnostics
        ));
    }

    let text_entries = text_report
        .entries
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let mut context = MigrationContext {
        source_sha256: source_sha256.to_owned(),
        source_version,
        source_format,
        text_entries,
        styles: Vec::new(),
        assets: Vec::new(),
        asset_ids_by_hash: BTreeMap::new(),
        diagnostics,
    };

    let artifact = match decoded {
        LegacyDecoded::Ddd(container) => {
            let default_font = context
                .normalized_text("document/default_font_name")
                .map(|entry| entry.decoded.text.clone())
                .unwrap_or_else(|| "Arial".to_owned());
            let defaults = DocumentDefaults {
                font_family: default_font,
                font_size_pt: container.defaults.default_font_size as f64,
                font_style_bits: container.defaults.default_font_style,
                object_shadows: container.defaults.object_shadows,
                auto_line_break: container.defaults.auto_line_break,
                connector_label_style: connector_label_style(
                    container.defaults.connector_label_style,
                ),
            };

            let mut master_layers = Vec::new();
            if let Some(stencil) = &container.stencil {
                if !stencil.objects.is_empty() {
                    let mut scene = Scene::default();
                    // Keep the legacy source path for text normalization, traceability and
                    // deterministic import IDs while mapping its semantics to a global
                    // renderer-independent master layer.
                    convert_object_list(
                        &stencil.objects,
                        "stencil",
                        true,
                        &mut scene,
                        &mut context,
                    )?;
                    master_layers.push(Layer {
                        id: context.layer_id("stencil"),
                        name: "Shared background".to_owned(),
                        visible: true,
                        locked: false,
                        draw_color: legacy_color(stencil.draw_color),
                        scene,
                    });
                    context.diagnostics.push(format!(
                        "legacy global stencil with {} top-level object(s) was imported as a document master layer rendered before page-local layers",
                        stencil.objects.len()
                    ));
                }
            }

            let mut pages = Vec::with_capacity(container.pages.len());
            for (page_index, page) in container.pages.iter().enumerate() {
                let page_path = format!("page/{page_index}");
                let page_name = context
                    .normalized_text(&format!("{page_path}/name"))
                    .map(|entry| entry.decoded.text.clone())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| format!("Page {}", page_index + 1));
                let mut layers = Vec::with_capacity(page.layers.len());
                for (layer_index, layer) in page.layers.iter().enumerate() {
                    let layer_path = format!("{page_path}/layer/{layer_index}");
                    let mut scene = Scene::default();
                    convert_object_list(
                        &layer.objects,
                        &layer_path,
                        true,
                        &mut scene,
                        &mut context,
                    )?;
                    layers.push(Layer {
                        id: context.layer_id(&layer_path),
                        name: format!("Layer {}", layer_index + 1),
                        visible: true,
                        locked: false,
                        draw_color: legacy_color(layer.draw_color),
                        scene,
                    });
                }
                pages.push(Page {
                    id: context.page_id(&page_path),
                    name: page_name,
                    size_mm: Size {
                        width: legacy_units(page.width),
                        height: legacy_units(page.height),
                    },
                    layers,
                });
            }

            let name = pages
                .first()
                .map(|page| page.name.clone())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Imported DiagramDesigner document".to_owned());
            NextArtifact::document(Document {
                id: context.document_id(),
                name,
                defaults,
                master_layers,
                pages,
                styles: std::mem::take(&mut context.styles),
                assets: std::mem::take(&mut context.assets),
                import: Some(context.import_metadata()),
            })
        }
        LegacyDecoded::Ddt(template) => {
            let mut scene = Scene::default();
            convert_object_list(
                &template.objects,
                "template",
                true,
                &mut scene,
                &mut context,
            )?;
            NextArtifact::template_palette(TemplatePalette {
                id: context.template_id(),
                name: "Imported DiagramDesigner template palette".to_owned(),
                size_mm: Size {
                    width: legacy_units(template.width),
                    height: legacy_units(template.height),
                },
                scene,
                styles: std::mem::take(&mut context.styles),
                assets: std::mem::take(&mut context.assets),
                import: Some(context.import_metadata()),
            })
        }
    };

    let validation = artifact.validate();
    if !validation.is_valid() {
        return Err(MigrationError::InvalidNextArtifact {
            issue_count: validation.issues.len(),
            issues: validation.issues,
        });
    }
    Ok(artifact)
}

fn import_namespace() -> Uuid {
    Uuid::from_u128(0x45d7_fa22_d993_4b30_9b4d_8d6c_68a9_3f52)
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn sha256_hex(bytes: &[u8]) -> String {
    bytes_to_hex(&Sha256::digest(bytes))
}

fn legacy_units(value: i32) -> f64 {
    value as f64 / LEGACY_UNITS_PER_MM
}

fn legacy_rect(rect: legacy_ddd::object::LegacyRect) -> Rect {
    let left = legacy_units(rect.left);
    let right = legacy_units(rect.right);
    let top = legacy_units(rect.top);
    let bottom = legacy_units(rect.bottom);
    Rect {
        x: left.min(right),
        y: top.min(bottom),
        width: (right - left).abs(),
        height: (bottom - top).abs(),
    }
}

fn legacy_point(x: i32, y: i32) -> Point {
    Point {
        x: legacy_units(x),
        y: legacy_units(y),
    }
}

fn normalized_point(point: LegacyFloatPoint) -> NormalizedPoint {
    NormalizedPoint {
        x: point.x,
        y: point.y,
    }
}

fn connector_label_style(value: u8) -> ConnectorLabelStyle {
    match value {
        0 => ConnectorLabelStyle::Transparent,
        1 => ConnectorLabelStyle::Solid,
        _ => ConnectorLabelStyle::Filled,
    }
}

fn marker_style(value: u16) -> MarkerStyle {
    match value {
        0x00 => MarkerStyle::None,
        0x11 => MarkerStyle::Stop,
        0x21 => MarkerStyle::Circle,
        0x22 => MarkerStyle::Ball,
        0x23 => MarkerStyle::Diamond,
        0x31 => MarkerStyle::Arrow1,
        0x32 => MarkerStyle::Arrow2,
        0x33 => MarkerStyle::Arrow3,
        0x41 => MarkerStyle::DoubleArrow,
        0x51 => MarkerStyle::UmlIsA,
        0x52 => MarkerStyle::UmlHasA,
        0x61 => MarkerStyle::Many,
        code => MarkerStyle::Custom(code),
    }
}

fn line_style(value: u16) -> LineStyle {
    match value {
        0x00 => LineStyle::Solid,
        0x11 => LineStyle::Dotted1,
        0x12 => LineStyle::Dotted2,
        0x21 => LineStyle::Short1,
        0x22 => LineStyle::Short2,
        0x31 => LineStyle::Long1,
        0x32 => LineStyle::Long2,
        0x41 => LineStyle::DashDot1,
        0x42 => LineStyle::DashDot2,
        0x51 => LineStyle::DashDash,
        0x61 => LineStyle::Outline,
        code => LineStyle::Custom(code),
    }
}

fn curve_kind(value: u8) -> CurveKind {
    match value {
        0 => CurveKind::CatmullRom,
        1 => CurveKind::Legacy,
        2 => CurveKind::Bezier,
        _ => CurveKind::LineSegments,
    }
}

fn legacy_color(value: i32) -> Option<Color> {
    if value == CL_NONE {
        return None;
    }
    let raw = value as u32;
    if raw & 0x8000_0000 != 0 {
        Some(Color::SystemPalette {
            index: (raw & 0xff) as u8,
        })
    } else {
        Some(Color::Rgba {
            r: (raw & 0xff) as u8,
            g: ((raw >> 8) & 0xff) as u8,
            b: ((raw >> 16) & 0xff) as u8,
            a: 0xff,
        })
    }
}

fn legacy_gradient(value: i32) -> Option<LinearGradient> {
    if value == CL_NONE {
        return None;
    }
    let raw = value as u32;
    let color = raw & 0x00ff_ffff;
    Some(LinearGradient {
        end_color: Color::Rgba {
            r: (color & 0xff) as u8,
            g: ((color >> 8) & 0xff) as u8,
            b: ((color >> 16) & 0xff) as u8,
            a: 0xff,
        },
        axis: if raw & 0x8000_0000 == 0 {
            GradientAxis::AlongX
        } else {
            GradientAxis::AlongY
        },
    })
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

fn object_line(object: &LegacyObject) -> Option<&LegacyLinePayload> {
    match object {
        LegacyObject::Rectangle { shape, .. }
        | LegacyObject::Ellipse { shape }
        | LegacyObject::Polygon { shape, .. }
        | LegacyObject::Flowchart { shape, .. } => Some(&shape.line),
        LegacyObject::StraightLine { connector }
        | LegacyObject::ConnectorLine { connector, .. } => Some(&connector.line),
        LegacyObject::CurveLine { base, .. } => match base {
            LegacyCurveBase::Line { line } => Some(line),
            LegacyCurveBase::Connector { connector } => Some(&connector.line),
        },
        LegacyObject::Text { .. }
        | LegacyObject::Bitmap { .. }
        | LegacyObject::Metafile { .. }
        | LegacyObject::Group { .. }
        | LegacyObject::InheritedLayer { .. } => None,
    }
}

fn object_shape(object: &LegacyObject) -> Option<&LegacyShapePayload> {
    match object {
        LegacyObject::Rectangle { shape, .. }
        | LegacyObject::Ellipse { shape }
        | LegacyObject::Polygon { shape, .. }
        | LegacyObject::Flowchart { shape, .. } => Some(shape),
        _ => None,
    }
}

fn object_connector(object: &LegacyObject) -> Option<&LegacyConnectorPayload> {
    match object {
        LegacyObject::StraightLine { connector }
        | LegacyObject::ConnectorLine { connector, .. } => Some(connector),
        LegacyObject::CurveLine {
            base: LegacyCurveBase::Connector { connector },
            ..
        } => Some(connector),
        _ => None,
    }
}

fn standard_shape_ports() -> Vec<NormalizedPoint> {
    vec![
        NormalizedPoint { x: 0.5, y: 0.5 },
        NormalizedPoint { x: 0.0, y: 0.5 },
        NormalizedPoint { x: 1.0, y: 0.5 },
        NormalizedPoint { x: 0.5, y: 0.0 },
        NormalizedPoint { x: 0.5, y: 1.0 },
    ]
}

fn port_positions(object: &LegacyObject) -> Vec<NormalizedPoint> {
    match object {
        LegacyObject::Text { .. } => Vec::new(),
        LegacyObject::Rectangle { custom_links, .. } => custom_links
            .as_ref()
            .map(|links| links.iter().copied().map(normalized_point).collect())
            .unwrap_or_else(standard_shape_ports),
        LegacyObject::Ellipse { .. } | LegacyObject::Flowchart { .. } => standard_shape_ports(),
        LegacyObject::StraightLine { .. }
        | LegacyObject::ConnectorLine { .. }
        | LegacyObject::CurveLine { .. } => vec![
            NormalizedPoint { x: 0.0, y: 0.0 },
            NormalizedPoint { x: 1.0, y: 1.0 },
        ],
        LegacyObject::Bitmap { picture, .. }
        | LegacyObject::Metafile { picture, .. }
        | LegacyObject::InheritedLayer { picture, .. } => picture
            .links
            .iter()
            .copied()
            .map(normalized_point)
            .collect(),
        LegacyObject::Group { links, .. } => links.iter().copied().map(normalized_point).collect(),
        LegacyObject::Polygon { points, .. } => {
            points.iter().copied().map(normalized_point).collect()
        }
    }
}

fn ports_for(object: &LegacyObject, element_id: ElementId) -> Vec<Port> {
    port_positions(object)
        .into_iter()
        .enumerate()
        .map(|(index, position)| Port {
            id: PortId::v5(element_id.0, &format!("port/{index}")),
            index: index as u16,
            position,
        })
        .collect()
}

fn anchor_set(bits: u8) -> AnchorSet {
    AnchorSet {
        left: bits & (1 << 0) != 0,
        right: bits & (1 << 1) != 0,
        top: bits & (1 << 2) != 0,
        bottom: bits & (1 << 3) != 0,
        horizontal_scale: bits & (1 << 4) != 0,
        vertical_scale: bits & (1 << 5) != 0,
    }
}

fn element_import(object: &LegacyObject, path: &str) -> ElementImportMetadata {
    let base = object_base(object);
    let mut raw_values = BTreeMap::from([
        ("left".to_owned(), base.position.left as i64),
        ("top".to_owned(), base.position.top as i64),
        ("right".to_owned(), base.position.right as i64),
        ("bottom".to_owned(), base.position.bottom as i64),
    ]);

    if let Some(text) = object_text(object) {
        raw_values.insert("text_x_align".to_owned(), text.text_x_align as i64);
        raw_values.insert("text_y_align".to_owned(), text.text_y_align as i64);
        raw_values.insert("text_color".to_owned(), text.text_color as i64);
        raw_values.insert("text_margin".to_owned(), text.margin as i64);
        raw_values.insert("text_angle_bits".to_owned(), text.angle.to_bits() as i64);
    }
    if let Some(line) = object_line(object) {
        raw_values.insert("line_width".to_owned(), line.line_width as i64);
        raw_values.insert("line_color".to_owned(), line.line_color as i64);
    }
    if let Some(shape) = object_shape(object) {
        raw_values.insert("fill_color".to_owned(), shape.fill_color as i64);
        raw_values.insert("gradient_color".to_owned(), shape.gradient_color as i64);
    }
    if let Some(connector) = object_connector(object) {
        raw_values.insert("start_marker".to_owned(), connector.start_marker as i64);
        raw_values.insert("end_marker".to_owned(), connector.end_marker as i64);
        raw_values.insert("line_style".to_owned(), connector.line_style as i64);
        raw_values.insert(
            "connector_fill_color".to_owned(),
            connector.fill_color as i64,
        );
    }

    ElementImportMetadata {
        source_path: path.to_owned(),
        source_type_id: object.legacy_type_id(),
        source_anchor_bits: base.anchors,
        raw_values,
    }
}

fn add_style(object: &LegacyObject, path: &str, context: &mut MigrationContext) -> Option<StyleId> {
    let text_color = object_text(object).and_then(|text| legacy_color(text.text_color));
    let stroke = object_line(object).and_then(|line| {
        legacy_color(line.line_color).map(|color| StrokeStyle {
            width_mm: legacy_units(line.line_width),
            color,
        })
    });
    let fill = object_shape(object).and_then(|shape| {
        legacy_color(shape.fill_color).map(|color| FillStyle {
            color,
            gradient: legacy_gradient(shape.gradient_color),
        })
    });

    if stroke.is_none() && fill.is_none() && text_color.is_none() {
        return None;
    }

    let id = context.style_id(&format!("{path}/style"));
    context.styles.push(ElementStyle {
        id,
        stroke,
        fill,
        text_color,
    });
    Some(id)
}

fn rich_text_for(path: &str, context: &MigrationContext) -> Option<RichTextDocument> {
    context
        .normalized_text(&format!("{path}/text"))
        .and_then(|entry| entry.rich_text.as_ref())
        .map(convert_rich_text)
}

fn text_layout(text: &LegacyTextPayload) -> TextLayout {
    let horizontal = match text.text_x_align {
        -1 => TextHorizontalAlignment::Left,
        0 => TextHorizontalAlignment::BlockLeft,
        1 => TextHorizontalAlignment::Right,
        2 => TextHorizontalAlignment::Center,
        3 => TextHorizontalAlignment::BlockRight,
        value => TextHorizontalAlignment::LegacyUnknown(value),
    };
    let vertical = match text.text_y_align {
        -1 => TextVerticalAlignment::Top,
        1 => TextVerticalAlignment::Bottom,
        0 | 2 | 3 => TextVerticalAlignment::Center,
        value => TextVerticalAlignment::LegacyUnknown(value),
    };
    TextLayout {
        horizontal,
        vertical,
        margin_mm: legacy_units(text.margin),
    }
}
fn convert_rich_text(document: &legacy_text::RichTextDocument) -> RichTextDocument {
    RichTextDocument {
        tokens: document.tokens.iter().map(convert_rich_token).collect(),
        tail: document.tail.as_ref().map(|tail| TextTailDirective {
            kind: match tail.kind {
                legacy_text::TailDirectiveKind::Action => TextTailKind::Action,
                legacy_text::TailDirectiveKind::Hint => TextTailKind::Hint,
            },
            value: tail.value.clone(),
        }),
        diagnostics: document
            .diagnostics
            .iter()
            .map(|diagnostic| {
                format!(
                    "character {}: {}",
                    diagnostic.char_offset, diagnostic.message
                )
            })
            .collect(),
    }
}

fn convert_rich_token(token: &legacy_text::RichTextToken) -> RichTextToken {
    match token {
        legacy_text::RichTextToken::Text { text, style } => RichTextToken::Text {
            text: text.clone(),
            style: convert_text_style(style),
        },
        legacy_text::RichTextToken::NewLine => RichTextToken::NewLine,
        legacy_text::RichTextToken::PageNumber { style } => RichTextToken::PageNumber {
            style: convert_text_style(style),
        },
        legacy_text::RichTextToken::PageCount { style } => RichTextToken::PageCount {
            style: convert_text_style(style),
        },
        legacy_text::RichTextToken::PageName { style } => RichTextToken::PageName {
            style: convert_text_style(style),
        },
        legacy_text::RichTextToken::SymbolGlyph {
            legacy_glyph,
            style,
        } => RichTextToken::SymbolGlyph {
            legacy_glyph: *legacy_glyph,
            style: convert_text_style(style),
        },
    }
}

fn convert_text_style(style: &legacy_text::RichTextStyle) -> TextStyle {
    TextStyle {
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        strikeout: style.strikeout,
        script: match style.script {
            legacy_text::ScriptPosition::Normal => ScriptPosition::Normal,
            legacy_text::ScriptPosition::Subscript => ScriptPosition::Subscript,
            legacy_text::ScriptPosition::Superscript => ScriptPosition::Superscript,
        },
        overline: style.overline,
        symbol_font: style.symbol_font,
        font_family: style.font_family.clone(),
        font_size_pt: style.font_size_pt,
        color: style.color_rgb.map(|rgb| Color::Rgba {
            r: ((rgb >> 16) & 0xff) as u8,
            g: ((rgb >> 8) & 0xff) as u8,
            b: (rgb & 0xff) as u8,
            a: 0xff,
        }),
    }
}

fn add_bitmap_asset(
    bitmap: &legacy_ddd::object::LegacyBitmapData,
    context: &mut MigrationContext,
) -> AssetId {
    let mut hasher = Sha256::new();
    hasher.update(bitmap.width.to_le_bytes());
    hasher.update(bitmap.height.to_le_bytes());
    hasher.update([bitmap.bits_per_pixel, bitmap.alpha_value]);
    if let Some(palette) = &bitmap.palette_raw {
        hasher.update(palette);
    }
    hasher.update(&bitmap.image_raw);
    if let Some(alpha) = &bitmap.alpha_raw {
        hasher.update(alpha);
    }
    let digest = hasher.finalize();
    let sha256 = bytes_to_hex(&digest);
    if let Some(id) = context.asset_ids_by_hash.get(&sha256) {
        return *id;
    }

    let id = AssetId::v5(import_namespace(), &format!("asset:{sha256}"));
    context.assets.push(Asset {
        id,
        sha256: sha256.clone(),
        media_type: "application/vnd.diagramdesigner-next.raster".to_owned(),
        payload: AssetPayload::Raster {
            width: bitmap.width,
            height: bitmap.height,
            bits_per_pixel: bitmap.bits_per_pixel,
            palette: bitmap.palette_raw.clone(),
            pixels: bitmap.image_raw.clone(),
            alpha: bitmap.alpha_raw.clone(),
            alpha_value: bitmap.alpha_value,
        },
    });
    context.asset_ids_by_hash.insert(sha256, id);
    id
}

fn add_binary_asset(bytes: &[u8], context: &mut MigrationContext) -> AssetId {
    let sha256 = sha256_hex(bytes);
    if let Some(id) = context.asset_ids_by_hash.get(&sha256) {
        return *id;
    }
    let id = AssetId::v5(import_namespace(), &format!("asset:{sha256}"));
    context.assets.push(Asset {
        id,
        sha256: sha256.clone(),
        media_type: "application/vnd.diagramdesigner-next.windows-metafile".to_owned(),
        payload: AssetPayload::Binary {
            bytes: bytes.to_vec(),
        },
    });
    context.asset_ids_by_hash.insert(sha256, id);
    id
}

fn convert_object_list(
    objects: &[LegacyObject],
    list_path: &str,
    add_to_roots: bool,
    scene: &mut Scene,
    context: &mut MigrationContext,
) -> Result<Vec<ElementId>, MigrationError> {
    let ids: Vec<ElementId> = (0..objects.len())
        .map(|index| context.element_id(&format!("{list_path}/object/{index}")))
        .collect();
    let ports: Vec<Vec<Port>> = objects
        .iter()
        .zip(ids.iter().copied())
        .map(|(object, id)| ports_for(object, id))
        .collect();

    if add_to_roots {
        scene.roots.extend(ids.iter().copied());
    }

    for (index, object) in objects.iter().enumerate() {
        let path = format!("{list_path}/object/{index}");
        let element = convert_object(object, &path, ids[index], &ids, &ports, context)?;
        scene.elements.push(element);

        if let LegacyObject::Group { children, .. } = object {
            convert_object_list(children, &format!("{path}/group"), false, scene, context)?;
        }
    }

    Ok(ids)
}

fn convert_object(
    object: &LegacyObject,
    path: &str,
    id: ElementId,
    owner_ids: &[ElementId],
    owner_ports: &[Vec<Port>],
    context: &mut MigrationContext,
) -> Result<Element, MigrationError> {
    let base = object_base(object);
    let name = context
        .normalized_text(&format!("{path}/name"))
        .map(|entry| entry.decoded.text.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("Legacy object {}", object.legacy_type_id()));
    let style_id = add_style(object, path, context);
    let text = object_text(object).and_then(|legacy_text| {
        rich_text_for(path, context).map(|content| TextBlock {
            content,
            layout: text_layout(legacy_text),
        })
    });
    let ports = ports_for(object, id);

    let kind = match object {
        LegacyObject::Text { .. } => ElementKind::Text,
        LegacyObject::Rectangle { corner_radius, .. } => ElementKind::Rectangle {
            corner_radius_mm: legacy_units(*corner_radius),
        },
        LegacyObject::Ellipse { .. } => ElementKind::Ellipse,
        LegacyObject::StraightLine { connector } => ElementKind::StraightConnector {
            connector: convert_connector(connector, path, owner_ids, owner_ports)?,
        },
        LegacyObject::ConnectorLine {
            connector,
            corner_radius,
        } => ElementKind::OrthogonalConnector {
            connector: convert_connector(connector, path, owner_ids, owner_ports)?,
            corner_radius_mm: legacy_units(*corner_radius),
        },
        LegacyObject::Bitmap { bitmap, .. } => ElementKind::Image {
            asset_id: add_bitmap_asset(bitmap, context),
        },
        LegacyObject::Metafile { metafile_raw, .. } => ElementKind::Metafile {
            asset_id: add_binary_asset(metafile_raw, context),
        },
        LegacyObject::Group { children, .. } => {
            let child_path = format!("{path}/group");
            ElementKind::Group {
                children: (0..children.len())
                    .map(|index| context.element_id(&format!("{child_path}/object/{index}")))
                    .collect(),
            }
        }
        LegacyObject::Polygon { points, .. } => ElementKind::Polygon {
            vertices: points.iter().copied().map(normalized_point).collect(),
        },
        LegacyObject::Flowchart { flowchart_type, .. } => ElementKind::Flowchart {
            shape_key: format!("builtin:diagramdesigner-flowchart/{flowchart_type}"),
        },
        LegacyObject::CurveLine {
            base,
            curve_type,
            points,
        } => ElementKind::Curve {
            curve_kind: curve_kind(*curve_type),
            connector: match base {
                LegacyCurveBase::Line { .. } => None,
                LegacyCurveBase::Connector { connector } => {
                    Some(convert_connector(connector, path, owner_ids, owner_ports)?)
                }
            },
            control_points_mm: points
                .iter()
                .map(|point| legacy_point(point.x, point.y))
                .collect(),
        },
        LegacyObject::InheritedLayer {
            relative_page_index,
            layer_index,
            ..
        } => ElementKind::LayerReference {
            relative_page_index: *relative_page_index,
            layer_index: *layer_index,
        },
    };

    let rotation_deg = match object {
        LegacyObject::Text { payload } => payload.angle as f64,
        LegacyObject::Metafile { angle, .. } => *angle as f64,
        _ => 0.0,
    };

    Ok(Element {
        id,
        name,
        bounds_mm: legacy_rect(base.position),
        rotation_deg,
        anchors: anchor_set(base.anchors),
        ports,
        style_id,
        text,
        kind,
        import: Some(element_import(object, path)),
    })
}

fn convert_connector(
    connector: &LegacyConnectorPayload,
    source_path: &str,
    owner_ids: &[ElementId],
    owner_ports: &[Vec<Port>],
) -> Result<Connector, MigrationError> {
    let position = connector.line.text.base.position;
    Ok(Connector {
        start: Endpoint {
            position_mm: legacy_point(position.left, position.top),
            connection: resolve_connection(
                &connector.links[0],
                source_path,
                owner_ids,
                owner_ports,
            )?,
        },
        end: Endpoint {
            position_mm: legacy_point(position.right, position.bottom),
            connection: resolve_connection(
                &connector.links[1],
                source_path,
                owner_ids,
                owner_ports,
            )?,
        },
        start_marker: marker_style(connector.start_marker),
        end_marker: marker_style(connector.end_marker),
        line_style: line_style(connector.line_style),
        secondary_color: legacy_color(connector.fill_color),
    })
}

fn resolve_connection(
    link: &legacy_ddd::object::LegacyLinkReference,
    source_path: &str,
    owner_ids: &[ElementId],
    owner_ports: &[Vec<Port>],
) -> Result<Option<Connection>, MigrationError> {
    if link.object_index == -1 {
        return Ok(None);
    }
    if link.object_index < 0 || link.object_index as usize >= owner_ids.len() {
        return Err(MigrationError::InvalidConnectorObjectIndex {
            source_path: source_path.to_owned(),
            object_index: link.object_index,
        });
    }
    let object_index = link.object_index as usize;
    let link_index = link.link_index.expect(
        "legacy codec guarantees a serialized link index for every non--1 object reference",
    );
    let Some(port) = owner_ports[object_index].get(link_index as usize) else {
        return Err(MigrationError::InvalidConnectorLinkIndex {
            source_path: source_path.to_owned(),
            object_index,
            link_index,
        });
    };
    Ok(Some(Connection {
        element_id: owner_ids[object_index],
        port_id: port.id,
    }))
}

#[cfg(test)]
mod tests {
    use legacy_ddd::template::LegacyTemplate;

    use super::*;

    #[test]
    fn converts_empty_template_to_valid_next_artifact() {
        let decoded = LegacyDecoded::Ddt(LegacyTemplate {
            width: 2520,
            height: 5040,
            objects: Vec::new(),
            trailing_bytes: 0,
        });
        let artifact = migrate_decoded(
            &decoded,
            LegacyFormat::Ddt,
            28,
            "0123456789abcdef",
            MigrationOptions::default(),
        )
        .unwrap();
        assert!(artifact.validate().is_valid());
        match artifact.artifact {
            next_domain::Artifact::TemplatePalette(template) => {
                assert_eq!(template.size_mm.width, 1.0);
                assert_eq!(template.size_mm.height, 2.0);
            }
            next_domain::Artifact::Document(_) => panic!("expected template palette"),
        }
    }

    #[test]
    fn maps_standard_shape_ports_in_source_order() {
        assert_eq!(
            standard_shape_ports(),
            vec![
                NormalizedPoint { x: 0.5, y: 0.5 },
                NormalizedPoint { x: 0.0, y: 0.5 },
                NormalizedPoint { x: 1.0, y: 0.5 },
                NormalizedPoint { x: 0.5, y: 0.0 },
                NormalizedPoint { x: 0.5, y: 1.0 },
            ]
        );
    }

    #[test]
    fn maps_legacy_marker_and_line_style_constants() {
        assert_eq!(marker_style(0x33), MarkerStyle::Arrow3);
        assert_eq!(marker_style(0x61), MarkerStyle::Many);
        assert_eq!(line_style(0x41), LineStyle::DashDot1);
        assert_eq!(line_style(0x61), LineStyle::Outline);
    }
}
