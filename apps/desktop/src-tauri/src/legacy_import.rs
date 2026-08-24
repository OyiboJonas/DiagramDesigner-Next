use std::path::Path;

use legacy_migrate::{MigrationOptions, migrate_bytes};
use next_domain::{
    Artifact, Document, DocumentDefaults, DocumentId, Layer, LayerId, NextArtifact, Page, PageId,
    TemplatePalette,
};

const MAX_LEGACY_INFLATED_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocumentSourceKind {
    NativeDdnx,
    LegacyDdd,
    LegacyDdt,
}

pub(crate) fn classify_path(path: &Path) -> Result<DocumentSourceKind, String> {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return Err("Select a .ddnx, .ddd or .ddt file.".to_owned());
    };
    match extension.to_ascii_lowercase().as_str() {
        "ddnx" => Ok(DocumentSourceKind::NativeDdnx),
        "ddd" => Ok(DocumentSourceKind::LegacyDdd),
        "ddt" => Ok(DocumentSourceKind::LegacyDdt),
        _ => Err("Select a .ddnx, .ddd or .ddt file.".to_owned()),
    }
}

pub(crate) fn migrate_legacy_bytes(
    bytes: &[u8],
    source_kind: DocumentSourceKind,
    defaults: DocumentDefaults,
) -> Result<NextArtifact, String> {
    let migrated = migrate_bytes(
        bytes,
        MAX_LEGACY_INFLATED_BYTES,
        MigrationOptions::default(),
    )
    .map_err(|error| error.to_string())?;

    match (source_kind, migrated.artifact) {
        (DocumentSourceKind::LegacyDdd, Artifact::Document(document)) => {
            Ok(NextArtifact::document(document))
        }
        (DocumentSourceKind::LegacyDdt, Artifact::TemplatePalette(palette)) => {
            Ok(materialize_palette_document(palette, defaults))
        }
        (DocumentSourceKind::LegacyDdd, Artifact::TemplatePalette(_)) => Err(
            "The selected .ddd file contains a template-palette payload instead of a document."
                .to_owned(),
        ),
        (DocumentSourceKind::LegacyDdt, Artifact::Document(_)) => Err(
            "The selected .ddt file contains a document payload instead of a template palette."
                .to_owned(),
        ),
        (DocumentSourceKind::NativeDdnx, _) => {
            Err("Native .ddnx packages do not pass through the legacy importer.".to_owned())
        }
    }
}

fn materialize_palette_document(
    palette: TemplatePalette,
    defaults: DocumentDefaults,
) -> NextArtifact {
    let page_id = PageId::new();
    let layer_id = LayerId::new();
    NextArtifact::document(Document {
        id: DocumentId::new(),
        name: palette.name.clone(),
        defaults,
        master_layers: Vec::new(),
        pages: vec![Page {
            id: page_id,
            name: palette.name,
            size_mm: palette.size_mm,
            layers: vec![Layer {
                id: layer_id,
                name: "Template palette".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: palette.scene,
            }],
        }],
        styles: palette.styles,
        assets: palette.assets,
        import: palette.import,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use next_domain::{
        Artifact, ConnectorLabelStyle, DocumentDefaults, ImportMetadata, Scene, Size, TemplateId,
        TemplatePalette,
    };

    use super::*;

    fn defaults() -> DocumentDefaults {
        DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Transparent,
        }
    }

    #[test]
    fn classifies_supported_document_extensions_case_insensitively() {
        assert_eq!(
            classify_path(Path::new("drawing.ddnx")).unwrap(),
            DocumentSourceKind::NativeDdnx
        );
        assert_eq!(
            classify_path(Path::new("drawing.DDD")).unwrap(),
            DocumentSourceKind::LegacyDdd
        );
        assert_eq!(
            classify_path(Path::new("palette.DdT")).unwrap(),
            DocumentSourceKind::LegacyDdt
        );
        assert!(classify_path(Path::new("drawing.svg")).is_err());
        assert!(classify_path(Path::new("drawing")).is_err());
    }

    #[test]
    fn materializes_template_palette_as_one_page_document_without_losing_metadata() {
        let size = Size {
            width: 123.0,
            height: 45.0,
        };
        let import = ImportMetadata {
            source_format: "ddt".to_owned(),
            source_version: 28,
            source_sha256: "abc123".to_owned(),
            importer: "test".to_owned(),
            diagnostics: vec!["kept".to_owned()],
        };
        let palette = TemplatePalette {
            id: TemplateId::new(),
            name: "Legacy palette".to_owned(),
            size_mm: size,
            scene: Scene::default(),
            styles: Vec::new(),
            assets: Vec::new(),
            import: Some(import.clone()),
        };

        let artifact = materialize_palette_document(palette, defaults());
        let Artifact::Document(document) = artifact.artifact else {
            panic!("palette adapter must return a document artifact");
        };
        assert_eq!(document.name, "Legacy palette");
        assert_eq!(document.import, Some(import));
        assert_eq!(document.pages.len(), 1);
        assert_eq!(document.pages[0].name, "Legacy palette");
        assert_eq!(document.pages[0].size_mm, size);
        assert_eq!(document.pages[0].layers.len(), 1);
        assert_eq!(document.pages[0].layers[0].scene, Scene::default());
        assert_eq!(document.defaults, defaults());
    }
}
