from pathlib import Path

VERSION = "0.1.0-alpha.1"


def replace_once(text, old, new, label):
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def patch(path, transform):
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    file.write_text(transform(text), encoding="utf-8")


# Desktop package version.
def patch_cargo(text):
    return replace_once(
        text,
        'name = "diagram-designer-next-desktop"\nversion = "0.1.0"',
        f'name = "diagram-designer-next-desktop"\nversion = "{VERSION}"',
        "desktop Cargo version",
    )

patch("apps/desktop/src-tauri/Cargo.toml", patch_cargo)


# Keep the pinned desktop lockfile synchronized without running an unlocked Cargo resolution.
def patch_lock(text):
    return replace_once(
        text,
        '[[package]]\nname = "diagram-designer-next-desktop"\nversion = "0.1.0"',
        f'[[package]]\nname = "diagram-designer-next-desktop"\nversion = "{VERSION}"',
        "desktop lock package version",
    )

patch("apps/desktop/src-tauri/Cargo.lock", patch_lock)


# Tauri runtime metadata / native title.
def patch_tauri_config(text):
    text = replace_once(
        text,
        '  "version": "0.1.0",',
        f'  "version": "{VERSION}",',
        "tauri app version",
    )
    return replace_once(
        text,
        '        "title": "DiagramDesigner Next",',
        '        "title": "DiagramDesigner Next Alpha",',
        "tauri main title",
    )

patch("apps/desktop/src-tauri/tauri.conf.json", patch_tauri_config)


# Rust desktop: expose version and guarantee dirty-close recovery at native boundary.
def patch_tauri_lib(text):
    text = replace_once(
        text,
        "    recovered: bool,\n    page_count: usize,",
        "    recovered: bool,\n    version: &'static str,\n    page_count: usize,",
        "state version DTO",
    )

    old_sync_start = '''#[tauri::command]
fn sync_recovery(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<RecoverySyncDto, CommandError> {
    let path = recovery_path(&app)?;
    let mut document = lock_document(&state)?;

'''
    new_sync_start = '''#[tauri::command]
fn sync_recovery(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<RecoverySyncDto, CommandError> {
    let mut document = lock_document(&state)?;
    sync_recovery_for_document(&app, &mut document)
}

fn sync_recovery_for_document(
    app: &AppHandle,
    document: &mut DesktopDocument,
) -> Result<RecoverySyncDto, CommandError> {
    let path = recovery_path(app)?;

'''
    text = replace_once(text, old_sync_start, new_sync_start, "recovery helper extraction")

    text = replace_once(
        text,
        '''fn reject_if_dirty(document: &DesktopDocument) -> Result<(), CommandError> {
    if document.recovered_dirty || document.imported_dirty || document.session.is_dirty() {
''',
        '''fn document_is_dirty(document: &DesktopDocument) -> bool {
    document.recovered_dirty || document.imported_dirty || document.session.is_dirty()
}

fn reject_if_dirty(document: &DesktopDocument) -> Result<(), CommandError> {
    if document_is_dirty(document) {
''',
        "dirty helper",
    )

    text = replace_once(
        text,
        "        dirty: document.recovered_dirty || document.imported_dirty || document.session.is_dirty(),\n        recovered: document.recovered_dirty,",
        "        dirty: document_is_dirty(document),\n        recovered: document.recovered_dirty,\n        version: env!(\"CARGO_PKG_VERSION\"),",
        "state dirty/version mapping",
    )

    close_helpers = r'''
fn report_close_checkpoint_error(app: &AppHandle, message: &str) {
    eprintln!("DiagramDesigner Next close blocked: {message}");
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let payload = serde_json::to_string(message).unwrap_or_else(|_| {
        "\"Unknown recovery checkpoint error\"".to_owned()
    });
    let _ = window.eval(format!(
        "window.diagramDesignerNext?.reportCloseCheckpointError({payload});"
    ));
}

fn checkpoint_dirty_document_before_close(
    window: &tauri::Window,
    api: &tauri::CloseRequestApi,
) {
    if window.label() != "main" {
        return;
    }
    let app = window.app_handle();
    let state = app.state::<DesktopState>();
    let mut document = match state.document.lock() {
        Ok(document) => document,
        Err(_) => {
            api.prevent_close();
            report_close_checkpoint_error(
                app,
                "The document state could not be locked for the final recovery checkpoint.",
            );
            return;
        }
    };
    if !document_is_dirty(&document) {
        return;
    }
    if let Err(error) = sync_recovery_for_document(app, &mut document) {
        api.prevent_close();
        report_close_checkpoint_error(app, &error.message);
    }
}

'''
    text = replace_once(
        text,
        "fn desktop_document_defaults() -> DocumentDefaults {",
        close_helpers + "fn desktop_document_defaults() -> DocumentDefaults {",
        "close checkpoint helpers",
    )

    text = replace_once(
        text,
        '''        .manage(state)
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
''',
        '''        .manage(state)
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                checkpoint_dirty_document_before_close(window, api);
            }
        })
        .invoke_handler(tauri::generate_handler![
''',
        "native close event",
    )
    return text

patch("apps/desktop/src-tauri/src/lib.rs", patch_tauri_lib)


# Alpha wording and version surfaces.
def patch_index(text):
    text = replace_once(
        text,
        '<span class="phase-label">Desktop editor preview</span>',
        '<span id="app-version" class="phase-label">Alpha · 0.1.0-alpha.1</span>',
        "alpha brand label",
    )
    return replace_once(
        text,
        '<span class="status-tech">Tauri 2 · DDNX · renderer candidate</span>',
        '<span id="status-tech" class="status-tech">Tauri 2 · DDNX · 0.1.0-alpha.1</span>',
        "alpha status metadata",
    )

patch("apps/desktop/ui/index.html", patch_index)


# UI: version-aware title, tested application shortcuts, close-checkpoint status reporting.
def patch_app_js(text):
    text = replace_once(
        text,
        "import { buildRulerTicks } from './editor-interaction/snapping.mjs';\n",
        "import { buildRulerTicks } from './editor-interaction/snapping.mjs';\nimport { isTextEditingTarget, resolveApplicationShortcut } from './editor-interaction/app-shortcuts.mjs';\n",
        "shortcut import",
    )
    text = replace_once(
        text,
        "  rendererStats: document.querySelector('#renderer-stats'),\n",
        "  rendererStats: document.querySelector('#renderer-stats'),\n  appVersion: document.querySelector('#app-version'),\n  statusTech: document.querySelector('#status-tech'),\n",
        "version DOM refs",
    )
    text = replace_once(
        text,
        "  elements.pageCount.textContent = String(state.pageCount);\n  elements.historyState.textContent = String(state.historyState);\n  document.title = `${state.dirty ? '● ' : ''}${state.name} — DiagramDesigner Next`;",
        "  elements.pageCount.textContent = String(state.pageCount);\n  elements.historyState.textContent = String(state.historyState);\n  elements.appVersion.textContent = `Alpha · ${state.version}`;\n  elements.statusTech.textContent = `Tauri 2 · DDNX · ${state.version}`;\n  document.title = `${state.dirty ? '● ' : ''}${state.name} — DiagramDesigner Next Alpha ${state.version}`;",
        "render version",
    )

    action_helpers = r'''
function saveCurrentDocument() {
  void runAction(
    'save_document',
    undefined,
    (result) => {
      const mode = result.commitMode === 'replaced' ? 'replaced atomically' : 'created atomically';
      return result.cleanupWarning
        ? `Saved (${mode}); temporary cleanup warning`
        : `Saved (${mode})`;
    },
    { syncRecovery: true },
  );
}

function undoCurrentDocument() {
  void runAction('undo', undefined, () => 'Undo', {
    syncRecovery: true,
    refreshPresentation: true,
  });
}

function redoCurrentDocument() {
  void runAction('redo', undefined, () => 'Redo', {
    syncRecovery: true,
    refreshPresentation: true,
  });
}

'''
    text = replace_once(
        text,
        "elements.saveDocument.addEventListener('click', () => {\n  void runAction(\n    'save_document',\n    undefined,\n    (result) => {\n      const mode = result.commitMode === 'replaced' ? 'replaced atomically' : 'created atomically';\n      return result.cleanupWarning\n        ? `Saved (${mode}); temporary cleanup warning`\n        : `Saved (${mode})`;\n    },\n    { syncRecovery: true },\n  );\n});\n\nelements.undo.addEventListener('click', () => {\n  void runAction('undo', undefined, () => 'Undo', {\n    syncRecovery: true,\n    refreshPresentation: true,\n  });\n});\n\nelements.redo.addEventListener('click', () => {\n  void runAction('redo', undefined, () => 'Redo', {\n    syncRecovery: true,\n    refreshPresentation: true,\n  });\n});\n",
        action_helpers + "elements.saveDocument.addEventListener('click', saveCurrentDocument);\n\nelements.undo.addEventListener('click', undoCurrentDocument);\n\nelements.redo.addEventListener('click', redoCurrentDocument);\n",
        "action helper extraction",
    )

    old_keydown = '''window.addEventListener(
  'keydown',
  (event) => {
    if (event.key !== 'Escape' || elements.recoveryDialog.open) {
      return;
    }
    if (svgSurface.cancelTransformGesture()) {
      setStatus('Transform cancelled');
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (svgSurface.cancelConnectorEndpointGesture()) {
      setStatus('Connector endpoint edit cancelled');
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (connectorTool !== null) {
      setConnectorTool(null);
      event.preventDefault();
      event.stopPropagation();
    }
  },
  true,
);
'''
    new_keydown = '''window.addEventListener(
  'keydown',
  (event) => {
    if (elements.recoveryDialog.open) {
      return;
    }

    if (!isBusy) {
      const shortcut = resolveApplicationShortcut(
        {
          key: event.key,
          ctrlKey: event.ctrlKey,
          metaKey: event.metaKey,
          shiftKey: event.shiftKey,
          altKey: event.altKey,
        },
        { textEditing: isTextEditingTarget(event.target) },
      );
      if (shortcut) {
        if (
          shortcut === 'delete-selection' &&
          Number(currentSelectionProperties?.count ?? 0) === 0
        ) {
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        if (shortcut === 'save') {
          saveCurrentDocument();
        } else if (shortcut === 'undo') {
          undoCurrentDocument();
        } else if (shortcut === 'redo') {
          redoCurrentDocument();
        } else if (shortcut === 'delete-selection') {
          void deleteCurrentSelection();
        }
        return;
      }
    }

    if (event.key !== 'Escape') {
      return;
    }
    if (svgSurface.cancelTransformGesture()) {
      setStatus('Transform cancelled');
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (svgSurface.cancelConnectorEndpointGesture()) {
      setStatus('Connector endpoint edit cancelled');
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (connectorTool !== null) {
      setConnectorTool(null);
      event.preventDefault();
      event.stopPropagation();
    }
  },
  true,
);
'''
    text = replace_once(text, old_keydown, new_keydown, "global shortcut handler")

    text = replace_once(
        text,
        "window.diagramDesignerNext = Object.freeze({\n  scheduleRecoverySync,\n  refreshPresentation,\n});",
        "window.diagramDesignerNext = Object.freeze({\n  scheduleRecoverySync,\n  refreshPresentation,\n  reportCloseCheckpointError(message) {\n    setStatus(`Close blocked: recovery checkpoint failed: ${String(message)}`);\n  },\n});",
        "close error bridge",
    )
    return text

patch("apps/desktop/ui/app.js", patch_app_js)


# Versioned alpha artifact from merged main.
def patch_preview_workflow(text):
    text = replace_once(
        text,
        "      - name: Assemble preview artifact\n        shell: powershell\n",
        "      - name: Resolve alpha version\n        shell: powershell\n        run: |\n          $version = (Get-Content \"apps/desktop/src-tauri/tauri.conf.json\" | ConvertFrom-Json).version\n          if (-not $version) { throw \"Desktop version is missing from tauri.conf.json\" }\n          \"DDN_DESKTOP_VERSION=$version\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append\n      - name: Assemble alpha artifact\n        shell: powershell\n",
        "alpha version step",
    )
    text = replace_once(
        text,
        '          Copy-Item $source "dist/windows-preview/DiagramDesigner-Next-Preview.exe"\n          Copy-Item "docs/testing/windows-preview.md" "dist/windows-preview/TESTING.md"',
        '          Copy-Item $source "dist/windows-preview/DiagramDesigner-Next-Alpha.exe"\n          Copy-Item "docs/testing/alpha-0.1.md" "dist/windows-preview/TESTING.md"',
        "alpha artifact files",
    )
    text = replace_once(
        text,
        '            "DiagramDesigner Next Windows Preview",\n            "source_commit=$commit",',
        '            "DiagramDesigner Next Windows Alpha",\n            "version=$env:DDN_DESKTOP_VERSION",\n            "channel=alpha",\n            "source_commit=$commit",',
        "alpha build metadata",
    )
    text = replace_once(
        text,
        "          name: DiagramDesigner-Next-Windows-Preview\n",
        "          name: DiagramDesigner-Next-Windows-Alpha-${{ env.DDN_DESKTOP_VERSION }}\n",
        "alpha artifact name",
    )
    return text

patch(".github/workflows/windows-preview.yml", patch_preview_workflow)


# Public status now describes the alpha instead of an internal renderer phase.
def patch_readme(text):
    text = replace_once(
        text,
        "> Status: Phase 1 — SVG selected as production renderer",
        "> Status: 0.1.0-alpha.1 candidate — Windows desktop alpha",
        "README status",
    )
    alpha_section = r'''
## 0.1 Windows alpha

The first alpha packages the current editor foundation into one functional Windows workflow: native DDNX open/save, migration-safe `.ddd`/`.ddt` import, crash recovery, pages/layers, basic shapes and text, straight/orthogonal connectors with ports, selection/move/snapping, direct resize/rotation, basic appearance, and Undo/Redo.

The alpha remains intentionally conservative around persistence: legacy sources are import-only, first-save overwrite is refused until a dedicated confirmation flow exists, and dirty window close writes a fresh atomic recovery checkpoint before the native window is allowed to close. The WebView still receives no broad filesystem or shell capability.

The unsigned portable Windows artifact and the exact alpha smoke path/known limitations are documented in [`docs/testing/alpha-0.1.md`](docs/testing/alpha-0.1.md).

'''
    return replace_once(
        text,
        "## Phase-1 renderer decision\n",
        alpha_section + "## Phase-1 renderer decision\n",
        "README alpha section",
    )

patch("README.md", patch_readme)
