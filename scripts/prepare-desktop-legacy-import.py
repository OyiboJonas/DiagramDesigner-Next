from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LIB = ROOT / "apps/desktop/src-tauri/src/lib.rs"
CARGO = ROOT / "apps/desktop/src-tauri/Cargo.toml"
APP_JS = ROOT / "apps/desktop/ui/app.js"
LEGACY = ROOT / "apps/desktop/src-tauri/src/legacy_import.rs"
DOC = ROOT / "docs/desktop-legacy-import.md"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


cargo = CARGO.read_text()
cargo = replace_once(
    cargo,
    'ddnx = { path = "../../../crates/ddnx" }\n',
    'ddnx = { path = "../../../crates/ddnx" }\nlegacy-migrate = { path = "../../../crates/legacy-migrate" }\n',
    "desktop legacy-migrate dependency",
)
CARGO.write_text(cargo)

legacy_source = r'''use std::path::Path;

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
'''
LEGACY.write_text(legacy_source)

lib = LIB.read_text()
lib = replace_once(
    lib,
    "mod renderer_benchmark_evidence;\n",
    "mod legacy_import;\nmod renderer_benchmark_evidence;\n",
    "legacy import module",
)
lib = replace_once(
    lib,
    "struct DesktopDocument {\n    session: ApplicationSession,\n    path: Option<PathBuf>,\n",
    "struct DesktopDocument {\n    session: ApplicationSession,\n    path: Option<PathBuf>,\n    /// Original legacy source path retained only for provenance/display. It is\n    /// never a save destination.\n    source_path: Option<PathBuf>,\n    /// A migrated legacy source starts as an unsaved Next copy even though its\n    /// freshly-created editor history has no edits yet.\n    imported_dirty: bool,\n",
    "desktop document import state",
)
lib = replace_once(
    lib,
    "            path: None,\n            recovered_dirty: false,\n",
    "            path: None,\n            source_path: None,\n            imported_dirty: false,\n            recovered_dirty: false,\n",
    "blank import state",
)
lib = replace_once(
    lib,
    "struct DocumentStateDto {\n    name: String,\n    path: Option<String>,\n    dirty: bool,\n",
    "struct DocumentStateDto {\n    name: String,\n    path: Option<String>,\n    source_path: Option<String>,\n    imported: bool,\n    dirty: bool,\n",
    "document state dto import fields",
)

open_start = lib.index("#[tauri::command]\nasync fn open_document(")
open_end = lib.index("\n#[tauri::command]\nasync fn save_document(", open_start)
new_open = r'''#[tauri::command]
async fn open_document(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DocumentActionDto, CommandError> {
    {
        let document = lock_document(&state)?;
        reject_if_dirty(&document)?;
    }

    let selected = app
        .dialog()
        .file()
        .add_filter("DiagramDesigner files", &["ddnx", "ddd", "ddt"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        let document = lock_document(&state)?;
        return Ok(DocumentActionDto {
            cancelled: true,
            state: document_state_dto(&document),
        });
    };
    let path = selected
        .into_path()
        .map_err(|error| CommandError::new("invalid_selected_path", error.to_string()))?;
    let source_kind = legacy_import::classify_path(&path)
        .map_err(|error| CommandError::new("unsupported_document_type", error))?;

    let bytes = fs::read(&path)
        .map_err(|error| CommandError::new("document_read_failed", error.to_string()))?;
    let (opened, persistent_path, source_path, imported_dirty) = match source_kind {
        legacy_import::DocumentSourceKind::NativeDdnx => {
            let opened = ApplicationSession::from_ddnx_bytes(&bytes, PackageLimits::default())
                .map_err(|error| CommandError::new("document_open_failed", error.to_string()))?;
            (opened, Some(path.clone()), None, false)
        }
        legacy_import::DocumentSourceKind::LegacyDdd
        | legacy_import::DocumentSourceKind::LegacyDdt => {
            let artifact = legacy_import::migrate_legacy_bytes(
                &bytes,
                source_kind,
                desktop_document_defaults(),
            )
            .map_err(|error| CommandError::new("legacy_import_failed", error))?;
            let opened = ApplicationSession::from_artifact(artifact)
                .map_err(|error| CommandError::new("legacy_import_failed", error.to_string()))?;
            // The legacy source is deliberately detached from persistence. Save
            // therefore enters the existing first-save .ddnx picker instead of
            // ever replacing the .ddd/.ddt source file.
            (opened, None, Some(path.clone()), true)
        }
    };

    let mut document = lock_document(&state)?;
    // A second check makes replacement safe even if another command edited the
    // document while the native picker or migration was running.
    reject_if_dirty(&document)?;
    *document = DesktopDocument {
        session: opened,
        path: persistent_path,
        source_path,
        imported_dirty,
        recovered_dirty: false,
    };

    Ok(DocumentActionDto {
        cancelled: false,
        state: document_state_dto(&document),
    })
}
'''
lib = lib[:open_start] + new_open + lib[open_end:]

lib = replace_once(
    lib,
    "    if first_save {\n        document.path = Some(destination);\n    }\n    document.recovered_dirty = false;\n",
    "    if first_save {\n        document.path = Some(destination);\n    }\n    document.imported_dirty = false;\n    document.recovered_dirty = false;\n",
    "save clears imported dirty state",
)
lib = replace_once(
    lib,
    "        path: None,\n        recovered_dirty: true,\n",
    "        path: None,\n        source_path: None,\n        imported_dirty: false,\n        recovered_dirty: true,\n",
    "recovery import state",
)
lib = replace_once(
    lib,
    "    if document.recovered_dirty {\n",
    "    if document.recovered_dirty || document.imported_dirty {\n",
    "recovery handles imported unsaved copy",
)
lib = replace_once(
    lib,
    "    if document.recovered_dirty || document.session.is_dirty() {\n",
    "    if document.recovered_dirty || document.imported_dirty || document.session.is_dirty() {\n",
    "dirty guard includes import state",
)

state_start = lib.index("fn document_state_dto(document: &DesktopDocument) -> DocumentStateDto {")
state_end = lib.index("\nfn normalize_ddnx_save_path", state_start)
new_state = r'''fn document_state_dto(document: &DesktopDocument) -> DocumentStateDto {
    let session = document.session.session();
    DocumentStateDto {
        name: session.document().name.clone(),
        path: document
            .path
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned()),
        source_path: document
            .source_path
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned()),
        imported: document.source_path.is_some(),
        dirty: document.recovered_dirty || document.imported_dirty || document.session.is_dirty(),
        recovered: document.recovered_dirty,
        page_count: session.document().pages.len(),
        active_page_id: session.active_page_id(),
        document_generation: document.session.document_generation(),
        history_state: session.current_history_state().value(),
    }
}
'''
lib = lib[:state_start] + new_state + lib[state_end:]

lib = replace_once(
    lib,
    "fn blank_document_artifact() -> NextArtifact {\n",
    r'''fn desktop_document_defaults() -> DocumentDefaults {
    DocumentDefaults {
        font_family: "Arial".to_owned(),
        font_size_pt: 10.0,
        font_style_bits: 0,
        object_shadows: false,
        auto_line_break: true,
        connector_label_style: ConnectorLabelStyle::Transparent,
    }
}

fn blank_document_artifact() -> NextArtifact {
''',
    "desktop document defaults helper",
)
lib = replace_once(
    lib,
    r'''        defaults: DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Transparent,
        },
''',
    "        defaults: desktop_document_defaults(),\n",
    "blank uses desktop defaults helper",
)
LIB.write_text(lib)

app = APP_JS.read_text()
state_start = app.index("function renderState(state) {")
state_end = app.index("\nfunction renderPresentationStats", state_start)
new_render_state = r'''function renderState(state) {
  elements.documentName.textContent = state.name;
  let pathLabel = 'Not saved yet';
  if (state.path && state.sourcePath) {
    pathLabel = `${state.path} · imported from ${state.sourcePath}`;
  } else if (state.path) {
    pathLabel = state.path;
  } else if (state.sourcePath) {
    pathLabel = `Imported from ${state.sourcePath} · save as .ddnx`;
  }
  elements.documentPath.textContent = pathLabel;
  elements.documentPath.title = pathLabel;

  if (state.recovered) {
    elements.documentDirty.textContent = 'Recovered — save required';
  } else if (state.imported && !state.path) {
    elements.documentDirty.textContent = 'Imported copy — save as DDNX';
  } else {
    elements.documentDirty.textContent = state.dirty ? 'Unsaved changes' : 'Saved';
  }

  elements.pageCount.textContent = String(state.pageCount);
  elements.historyState.textContent = String(state.historyState);
  document.title = `${state.dirty ? '● ' : ''}${state.name} — DiagramDesigner Next`;
}
'''
app = app[:state_start] + new_render_state + app[state_end:]
app = replace_once(
    app,
    "  void runAction('open_document', undefined, () => 'Document opened', {\n",
    "  void runAction(\n    'open_document',\n    undefined,\n    (result) =>\n      result.state?.imported\n        ? 'Legacy file imported as an unsaved Next copy'\n        : 'Document opened',\n    {\n",
    "legacy open status message",
)
app = replace_once(
    app,
    "    preserveSelection: false,\n  });\n});\n\nelements.saveDocument.addEventListener",
    "      preserveSelection: false,\n    },\n  );\n});\n\nelements.saveDocument.addEventListener",
    "close multiline open action",
)
APP_JS.write_text(app)

DOC.write_text(r'''# Desktop legacy import contract

The desktop **Open** workflow accepts native `.ddnx` packages and legacy DiagramDesigner `.ddd` / `.ddt` sources.

## Persistence boundary

Native `.ddnx` opens keep their selected path as the persistence target. Legacy files are different: they are read-only import sources. The desktop stores their path only as transient provenance/display state and deliberately leaves the editable document persistence path empty. The existing first-save flow therefore requires a new `.ddnx` destination and cannot overwrite the legacy source.

An imported copy owns an explicit desktop `imported_dirty` state because the newly-created editor session itself correctly considers its initial history state clean. That desktop state keeps the imported copy visibly unsaved and eligible for recovery checkpoints until a `.ddnx` save succeeds.

## DDD and DDT mapping

`.ddd` migration must produce a Next document artifact. `.ddt` migration must produce a Next template palette; the desktop then explicitly materializes that palette as a one-page editable document. Palette size, scene, styles, assets and import metadata are preserved. Fresh document/page/layer IDs are created and the normal desktop document defaults are applied.

The extension selects the expected legacy artifact family. A mismatched payload is rejected instead of being silently reinterpreted.

## Recovery and provenance

The original legacy source path is never serialized into the Next document. This avoids persisting machine-local filesystem paths. It is retained in the live desktop state for provenance/display while the session is running. Recovery snapshots remain normal `.ddnx` document snapshots; restoring one is therefore intentionally detached from the original source path and follows the existing recovered-copy Save As contract.
''')

print("Prepared desktop legacy import source changes")
