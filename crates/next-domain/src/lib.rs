use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;
pub const LEGACY_UNITS_PER_MM: f64 = 2520.0;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn v5(namespace: Uuid, name: &str) -> Self {
                Self(Uuid::new_v5(&namespace, name.as_bytes()))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

id_type!(DocumentId);
id_type!(TemplateId);
id_type!(PageId);
id_type!(LayerId);
id_type!(ElementId);
id_type!(PortId);
id_type!(StyleId);
id_type!(AssetId);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NextArtifact {
    pub schema_version: u32,
    pub artifact: Artifact,
}

impl NextArtifact {
    pub fn document(document: Document) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            artifact: Artifact::Document(document),
        }
    }

    pub fn template_palette(template: TemplatePalette) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            artifact: Artifact::TemplatePalette(template),
        }
    }

    pub fn validate(&self) -> ValidationReport {
        let mut report = ValidationReport::default();
        if self.schema_version != SCHEMA_VERSION {
            report.issues.push(ValidationIssue::SchemaVersion {
                expected: SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }

        match &self.artifact {
            Artifact::Document(document) => validate_document(document, &mut report),
            Artifact::TemplatePalette(template) => validate_template(template, &mut report),
        }
        report
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Artifact {
    Document(Document),
    TemplatePalette(TemplatePalette),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub name: String,
    pub defaults: DocumentDefaults,
    /// Shared background/master layers rendered before page-local layers on every
    /// page.
    ///
    /// This is a global document concept, not a legacy-import special case. Legacy
    /// DDD `Stencil` content maps here because the original renderer draws it before
    /// every page's local layers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub master_layers: Vec<Layer>,
    pub pages: Vec<Page>,
    pub styles: Vec<ElementStyle>,
    pub assets: Vec<Asset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import: Option<ImportMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplatePalette {
    pub id: TemplateId,
    pub name: String,
    pub size_mm: Size,
    pub scene: Scene,
    pub styles: Vec<ElementStyle>,
    pub assets: Vec<Asset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import: Option<ImportMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentDefaults {
    pub font_family: String,
    pub font_size_pt: f64,
    pub font_style_bits: i32,
    pub object_shadows: bool,
    pub auto_line_break: bool,
    pub connector_label_style: ConnectorLabelStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorLabelStyle {
    Transparent,
    Solid,
    Filled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: PageId,
    pub name: String,
    pub size_mm: Size,
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draw_color: Option<Color>,
    pub scene: Scene,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Scene {
    /// Z-order of top-level elements. Group children are excluded from this list.
    pub roots: Vec<ElementId>,
    /// Flat storage; groups reference children by stable IDs.
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: ElementId,
    pub name: String,
    pub bounds_mm: Rect,
    pub rotation_deg: f64,
    pub anchors: AnchorSet,
    pub ports: Vec<Port>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_id: Option<StyleId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextBlock>,
    pub kind: ElementKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import: Option<ElementImportMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ElementKind {
    Text,
    Rectangle {
        corner_radius_mm: f64,
    },
    Ellipse,
    StraightConnector {
        connector: Connector,
    },
    OrthogonalConnector {
        connector: Connector,
        corner_radius_mm: f64,
    },
    Image {
        asset_id: AssetId,
    },
    Metafile {
        asset_id: AssetId,
    },
    Group {
        children: Vec<ElementId>,
    },
    Polygon {
        vertices: Vec<NormalizedPoint>,
    },
    Flowchart {
        shape_key: String,
    },
    Curve {
        curve_kind: CurveKind,
        #[serde(skip_serializing_if = "Option::is_none")]
        connector: Option<Connector>,
        control_points_mm: Vec<Point>,
    },
    LayerReference {
        relative_page_index: i32,
        layer_index: i32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveKind {
    CatmullRom,
    Legacy,
    Bezier,
    LineSegments,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    pub start: Endpoint,
    pub end: Endpoint,
    pub start_marker: MarkerStyle,
    pub end_marker: MarkerStyle,
    pub line_style: LineStyle,
    /// Secondary rendering colour used by legacy outlined lines and marker interiors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_color: Option<Color>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Endpoint {
    /// Free/source position is always retained even when the endpoint is connected.
    pub position_mm: Point,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<Connection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub element_id: ElementId,
    pub port_id: PortId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "code", rename_all = "snake_case")]
pub enum MarkerStyle {
    None,
    Stop,
    Circle,
    Ball,
    Diamond,
    Arrow1,
    Arrow2,
    Arrow3,
    DoubleArrow,
    UmlIsA,
    UmlHasA,
    Many,
    Custom(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "code", rename_all = "snake_case")]
pub enum LineStyle {
    Solid,
    Dotted1,
    Dotted2,
    Short1,
    Short2,
    Long1,
    Long2,
    DashDot1,
    DashDot2,
    DashDash,
    Outline,
    Custom(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Port {
    pub id: PortId,
    /// Stable logical order within the element. Legacy link indices map here at import.
    pub index: u16,
    pub position: NormalizedPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct AnchorSet {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
    pub horizontal_scale: bool,
    pub vertical_scale: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct NormalizedPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementStyle {
    pub id: StyleId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<StrokeStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<FillStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<Color>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrokeStyle {
    pub width_mm: f64,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FillStyle {
    pub color: Color,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient: Option<LinearGradient>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearGradient {
    pub end_color: Color,
    pub axis: GradientAxis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientAxis {
    AlongX,
    AlongY,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Color {
    Rgba {
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    },
    /// Explicit unresolved platform/system palette entry. Renderers must choose a
    /// platform-independent fallback instead of interpreting the raw integer ad hoc.
    SystemPalette {
        index: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: AssetId,
    pub sha256: String,
    pub media_type: String,
    pub payload: AssetPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssetPayload {
    Raster {
        width: i32,
        height: i32,
        bits_per_pixel: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        palette: Option<Vec<u8>>,
        pixels: Vec<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        alpha: Option<Vec<u8>>,
        alpha_value: u8,
    },
    Binary {
        bytes: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportMetadata {
    pub source_format: String,
    pub source_version: u16,
    pub source_sha256: String,
    pub importer: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementImportMetadata {
    pub source_path: String,
    pub source_type_id: u8,
    pub source_anchor_bits: u8,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub raw_values: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBlock {
    pub content: RichTextDocument,
    pub layout: TextLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TextLayout {
    pub horizontal: TextHorizontalAlignment,
    pub vertical: TextVerticalAlignment,
    pub margin_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "legacy_value", rename_all = "snake_case")]
pub enum TextHorizontalAlignment {
    Left,
    BlockLeft,
    Right,
    Center,
    BlockRight,
    LegacyUnknown(i8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "legacy_value", rename_all = "snake_case")]
pub enum TextVerticalAlignment {
    Top,
    Center,
    Bottom,
    LegacyUnknown(i8),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RichTextDocument {
    pub tokens: Vec<RichTextToken>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail: Option<TextTailDirective>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextToken {
    Text {
        text: String,
        style: TextStyle,
    },
    NewLine,
    PageNumber {
        style: TextStyle,
    },
    PageCount {
        style: TextStyle,
    },
    PageName {
        style: TextStyle,
    },
    SymbolGlyph {
        legacy_glyph: char,
        style: TextStyle,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TextStyle {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScriptPosition {
    #[default]
    Normal,
    Subscript,
    Superscript,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextTailDirective {
    pub kind: TextTailKind,
    /// Inert imported value. No renderer/editor component is allowed to execute it implicitly.
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextTailKind {
    Action,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValidationIssue {
    SchemaVersion {
        expected: u32,
        actual: u32,
    },
    DuplicatePageId {
        page_id: PageId,
    },
    DuplicateLayerId {
        layer_id: LayerId,
    },
    DuplicateElementId {
        element_id: ElementId,
    },
    DuplicatePortId {
        port_id: PortId,
    },
    DuplicateStyleId {
        style_id: StyleId,
    },
    DuplicateAssetId {
        asset_id: AssetId,
    },
    MissingRootElement {
        layer_id: LayerId,
        element_id: ElementId,
    },
    MissingGroupChild {
        group_id: ElementId,
        child_id: ElementId,
    },
    MissingStyle {
        element_id: ElementId,
        style_id: StyleId,
    },
    MissingAsset {
        element_id: ElementId,
        asset_id: AssetId,
    },
    MissingConnectionElement {
        source_element_id: ElementId,
        target_element_id: ElementId,
    },
    MissingConnectionPort {
        source_element_id: ElementId,
        target_element_id: ElementId,
        port_id: PortId,
    },
}

fn validate_document(document: &Document, report: &mut ValidationReport) {
    let mut page_ids = BTreeSet::new();
    let mut layer_ids = BTreeSet::new();
    let mut element_ids = BTreeSet::new();
    let mut port_ids = BTreeSet::new();
    let style_ids = validate_styles(&document.styles, report);
    let asset_ids = validate_assets(&document.assets, report);

    // Master layers and page-local layers share the same identity namespaces. An
    // element may therefore move between layers without acquiring a new identity.
    for layer in &document.master_layers {
        if !layer_ids.insert(layer.id) {
            report
                .issues
                .push(ValidationIssue::DuplicateLayerId { layer_id: layer.id });
        }
        validate_scene(
            layer.id,
            &layer.scene,
            &style_ids,
            &asset_ids,
            &mut element_ids,
            &mut port_ids,
            report,
        );
    }

    for page in &document.pages {
        if !page_ids.insert(page.id) {
            report
                .issues
                .push(ValidationIssue::DuplicatePageId { page_id: page.id });
        }
        for layer in &page.layers {
            if !layer_ids.insert(layer.id) {
                report
                    .issues
                    .push(ValidationIssue::DuplicateLayerId { layer_id: layer.id });
            }
            validate_scene(
                layer.id,
                &layer.scene,
                &style_ids,
                &asset_ids,
                &mut element_ids,
                &mut port_ids,
                report,
            );
        }
    }
}

fn validate_template(template: &TemplatePalette, report: &mut ValidationReport) {
    let style_ids = validate_styles(&template.styles, report);
    let asset_ids = validate_assets(&template.assets, report);
    let mut element_ids = BTreeSet::new();
    let mut port_ids = BTreeSet::new();
    validate_scene(
        LayerId::v5(template.id.0, "template-scene"),
        &template.scene,
        &style_ids,
        &asset_ids,
        &mut element_ids,
        &mut port_ids,
        report,
    );
}

fn validate_styles(styles: &[ElementStyle], report: &mut ValidationReport) -> BTreeSet<StyleId> {
    let mut ids = BTreeSet::new();
    for style in styles {
        if !ids.insert(style.id) {
            report
                .issues
                .push(ValidationIssue::DuplicateStyleId { style_id: style.id });
        }
    }
    ids
}

fn validate_assets(assets: &[Asset], report: &mut ValidationReport) -> BTreeSet<AssetId> {
    let mut ids = BTreeSet::new();
    for asset in assets {
        if !ids.insert(asset.id) {
            report
                .issues
                .push(ValidationIssue::DuplicateAssetId { asset_id: asset.id });
        }
    }
    ids
}

fn validate_scene(
    layer_id: LayerId,
    scene: &Scene,
    style_ids: &BTreeSet<StyleId>,
    asset_ids: &BTreeSet<AssetId>,
    element_ids: &mut BTreeSet<ElementId>,
    port_ids: &mut BTreeSet<PortId>,
    report: &mut ValidationReport,
) {
    let mut elements = BTreeMap::new();
    for element in &scene.elements {
        if !element_ids.insert(element.id) {
            report.issues.push(ValidationIssue::DuplicateElementId {
                element_id: element.id,
            });
        }
        elements.entry(element.id).or_insert(element);

        for port in &element.ports {
            if !port_ids.insert(port.id) {
                report
                    .issues
                    .push(ValidationIssue::DuplicatePortId { port_id: port.id });
            }
        }

        if let Some(style_id) = element.style_id {
            if !style_ids.contains(&style_id) {
                report.issues.push(ValidationIssue::MissingStyle {
                    element_id: element.id,
                    style_id,
                });
            }
        }
    }

    for root in &scene.roots {
        if !elements.contains_key(root) {
            report.issues.push(ValidationIssue::MissingRootElement {
                layer_id,
                element_id: *root,
            });
        }
    }

    for element in &scene.elements {
        match &element.kind {
            ElementKind::Group { children } => {
                for child in children {
                    if !elements.contains_key(child) {
                        report.issues.push(ValidationIssue::MissingGroupChild {
                            group_id: element.id,
                            child_id: *child,
                        });
                    }
                }
            }
            ElementKind::Image { asset_id } | ElementKind::Metafile { asset_id } => {
                if !asset_ids.contains(asset_id) {
                    report.issues.push(ValidationIssue::MissingAsset {
                        element_id: element.id,
                        asset_id: *asset_id,
                    });
                }
            }
            _ => {}
        }

        let connector = match &element.kind {
            ElementKind::StraightConnector { connector }
            | ElementKind::OrthogonalConnector { connector, .. } => Some(connector),
            ElementKind::Curve {
                connector: Some(connector),
                ..
            } => Some(connector),
            _ => None,
        };

        if let Some(connector) = connector {
            validate_endpoint(element.id, &connector.start, &elements, report);
            validate_endpoint(element.id, &connector.end, &elements, report);
        }
    }
}

fn validate_endpoint(
    source_element_id: ElementId,
    endpoint: &Endpoint,
    elements: &BTreeMap<ElementId, &Element>,
    report: &mut ValidationReport,
) {
    let Some(connection) = endpoint.connection else {
        return;
    };
    let Some(target) = elements.get(&connection.element_id) else {
        report
            .issues
            .push(ValidationIssue::MissingConnectionElement {
                source_element_id,
                target_element_id: connection.element_id,
            });
        return;
    };
    if !target
        .ports
        .iter()
        .any(|port| port.id == connection.port_id)
    {
        report.issues.push(ValidationIssue::MissingConnectionPort {
            source_element_id,
            target_element_id: connection.element_id,
            port_id: connection.port_id,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_defaults() -> DocumentDefaults {
        DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Solid,
        }
    }

    fn text_element(id: ElementId, ports: Vec<Port>) -> Element {
        Element {
            id,
            name: "Text".to_owned(),
            bounds_mm: Rect::default(),
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports,
            style_id: None,
            text: None,
            kind: ElementKind::Text,
            import: None,
        }
    }

    fn layer_with_element(id: LayerId, element: Element) -> Layer {
        let element_id = element.id;
        Layer {
            id,
            name: "Layer".to_owned(),
            visible: true,
            locked: false,
            draw_color: None,
            scene: Scene {
                roots: vec![element_id],
                elements: vec![element],
            },
        }
    }

    #[test]
    fn validates_a_minimal_document() {
        let document_id = DocumentId::new();
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let element_id = ElementId::new();
        let artifact = NextArtifact::document(Document {
            id: document_id,
            name: "Example".to_owned(),
            defaults: test_defaults(),
            master_layers: Vec::new(),
            pages: vec![Page {
                id: page_id,
                name: "Page 1".to_owned(),
                size_mm: Size {
                    width: 210.0,
                    height: 297.0,
                },
                layers: vec![layer_with_element(
                    layer_id,
                    text_element(element_id, Vec::new()),
                )],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        });

        assert!(artifact.validate().is_valid());
    }

    #[test]
    fn rejects_duplicate_layer_id_across_master_and_page_layers() {
        let layer_id = LayerId::new();
        let artifact = NextArtifact::document(Document {
            id: DocumentId::new(),
            name: "Duplicate layer test".to_owned(),
            defaults: test_defaults(),
            master_layers: vec![Layer {
                id: layer_id,
                name: "Master".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene::default(),
            }],
            pages: vec![Page {
                id: PageId::new(),
                name: "Page 1".to_owned(),
                size_mm: Size::default(),
                layers: vec![Layer {
                    id: layer_id,
                    name: "Page layer".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene::default(),
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        });

        assert!(matches!(
            artifact.validate().issues.as_slice(),
            [ValidationIssue::DuplicateLayerId {
                layer_id: duplicate
            }] if *duplicate == layer_id
        ));
    }

    #[test]
    fn rejects_duplicate_element_id_across_master_and_page_layers() {
        let element_id = ElementId::new();
        let artifact = NextArtifact::document(Document {
            id: DocumentId::new(),
            name: "Duplicate element test".to_owned(),
            defaults: test_defaults(),
            master_layers: vec![layer_with_element(
                LayerId::new(),
                text_element(element_id, Vec::new()),
            )],
            pages: vec![Page {
                id: PageId::new(),
                name: "Page 1".to_owned(),
                size_mm: Size::default(),
                layers: vec![layer_with_element(
                    LayerId::new(),
                    text_element(element_id, Vec::new()),
                )],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        });

        assert!(matches!(
            artifact.validate().issues.as_slice(),
            [ValidationIssue::DuplicateElementId {
                element_id: duplicate
            }] if *duplicate == element_id
        ));
    }

    #[test]
    fn rejects_duplicate_port_id_across_master_and_page_layers() {
        let port_id = PortId::new();
        let port = Port {
            id: port_id,
            index: 0,
            position: NormalizedPoint::default(),
        };
        let artifact = NextArtifact::document(Document {
            id: DocumentId::new(),
            name: "Duplicate port test".to_owned(),
            defaults: test_defaults(),
            master_layers: vec![layer_with_element(
                LayerId::new(),
                text_element(ElementId::new(), vec![port]),
            )],
            pages: vec![Page {
                id: PageId::new(),
                name: "Page 1".to_owned(),
                size_mm: Size::default(),
                layers: vec![layer_with_element(
                    LayerId::new(),
                    text_element(ElementId::new(), vec![port]),
                )],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        });

        assert!(matches!(
            artifact.validate().issues.as_slice(),
            [ValidationIssue::DuplicatePortId { port_id: duplicate }] if *duplicate == port_id
        ));
    }

    #[test]
    fn rejects_missing_group_children() {
        let layer_id = LayerId::new();
        let group_id = ElementId::new();
        let missing = ElementId::new();
        let scene = Scene {
            roots: vec![group_id],
            elements: vec![Element {
                id: group_id,
                name: "Group".to_owned(),
                bounds_mm: Rect::default(),
                rotation_deg: 0.0,
                anchors: AnchorSet::default(),
                ports: Vec::new(),
                style_id: None,
                text: None,
                kind: ElementKind::Group {
                    children: vec![missing],
                },
                import: None,
            }],
        };
        let mut report = ValidationReport::default();
        let mut element_ids = BTreeSet::new();
        let mut port_ids = BTreeSet::new();
        validate_scene(
            layer_id,
            &scene,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &mut element_ids,
            &mut port_ids,
            &mut report,
        );
        assert!(matches!(
            report.issues.as_slice(),
            [ValidationIssue::MissingGroupChild { .. }]
        ));
    }
}
