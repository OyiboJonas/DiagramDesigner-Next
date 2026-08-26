#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding='utf-8')
    count = text.count(old)
    if count != 1:
        raise SystemExit(f'{path}: expected one match, found {count}: {old[:80]!r}')
    path.write_text(text.replace(old, new, 1), encoding='utf-8')


# Deterministic save-intent policy, independent of native dialogs/filesystem.
save_policy = ROOT / 'apps/desktop/src-tauri/src/save_policy.rs'
save_policy.write_text('''#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaveIntent {
    Save,
    SaveAs,
}

impl SaveIntent {
    pub(crate) fn uses_existing_path(self, has_existing_path: bool) -> bool {
        matches!(self, Self::Save) && has_existing_path
    }

    pub(crate) fn requires_overwrite_confirmation(
        self,
        has_existing_path: bool,
        destination_exists: bool,
    ) -> bool {
        destination_exists && !self.uses_existing_path(has_existing_path)
    }

    pub(crate) fn updates_persistent_path(self, has_existing_path: bool) -> bool {
        !self.uses_existing_path(has_existing_path)
    }
}

#[cfg(test)]
mod tests {
    use super::SaveIntent;

    #[test]
    fn normal_save_reuses_existing_native_path_without_confirmation() {
        assert!(SaveIntent::Save.uses_existing_path(true));
        assert!(!SaveIntent::Save.requires_overwrite_confirmation(true, true));
        assert!(!SaveIntent::Save.updates_persistent_path(true));
    }

    #[test]
    fn first_save_uses_picker_and_confirms_existing_destination() {
        assert!(!SaveIntent::Save.uses_existing_path(false));
        assert!(SaveIntent::Save.requires_overwrite_confirmation(false, true));
        assert!(SaveIntent::Save.updates_persistent_path(false));
    }

    #[test]
    fn save_as_always_uses_picker_and_confirms_existing_destination() {
        assert!(!SaveIntent::SaveAs.uses_existing_path(true));
        assert!(SaveIntent::SaveAs.requires_overwrite_confirmation(true, true));
        assert!(SaveIntent::SaveAs.updates_persistent_path(true));
    }

    #[test]
    fn newly_selected_nonexistent_destination_needs_no_confirmation() {
        assert!(!SaveIntent::Save.requires_overwrite_confirmation(false, false));
        assert!(!SaveIntent::SaveAs.requires_overwrite_confirmation(true, false));
    }
}
''', encoding='utf-8')

lib = ROOT / 'apps/desktop/src-tauri/src/lib.rs'
replace_once(lib, 'mod renderer_benchmark_evidence;\n', 'mod renderer_benchmark_evidence;\nmod save_policy;\n')
replace_once(
    lib,
    'use tauri_plugin_dialog::DialogExt;\n',
    'use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};\n\nuse save_policy::SaveIntent;\n',
)

text = lib.read_text(encoding='utf-8')
start = text.index('#[tauri::command]\nasync fn save_document(')
end = text.index('#[tauri::command]\nfn recovery_status', start)
new_save = '''#[tauri::command]
async fn save_document(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<SaveResultDto, CommandError> {
    save_document_with_intent(app, state, SaveIntent::Save).await
}

#[tauri::command]
async fn save_as_document(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<SaveResultDto, CommandError> {
    save_document_with_intent(app, state, SaveIntent::SaveAs).await
}

async fn save_document_with_intent(
    app: AppHandle,
    state: State<'_, DesktopState>,
    intent: SaveIntent,
) -> Result<SaveResultDto, CommandError> {
    let (prepared, existing_path) = {
        let document = lock_document(&state)?;
        let prepared = document
            .session
            .prepare_document_save(PackageLimits::default())
            .map_err(|error| {
                CommandError::new("document_prepare_save_failed", error.to_string())
            })?;
        (prepared, document.path.clone())
    };

    let has_existing_path = existing_path.is_some();
    let destination = if intent.uses_existing_path(has_existing_path) {
        existing_path.clone().ok_or_else(|| {
            CommandError::new(
                "save_path_missing",
                "The existing document save path is no longer available.",
            )
        })?
    } else {
        let suggested_name = existing_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled.ddnx".to_owned());
        let selected = app
            .dialog()
            .file()
            .add_filter("DiagramDesigner Next", &["ddnx"])
            .set_file_name(&suggested_name)
            .blocking_save_file();
        let Some(selected) = selected else {
            let document = lock_document(&state)?;
            return Ok(cancelled_save_result(&document));
        };
        normalize_ddnx_save_path(
            selected
                .into_path()
                .map_err(|error| CommandError::new("invalid_selected_path", error.to_string()))?,
        )?
    };

    if intent.requires_overwrite_confirmation(has_existing_path, destination.exists()) {
        // This command is async, matching the existing native file-picker boundary;
        // the blocking native dialog therefore does not execute in a synchronous
        // main-thread Tauri command context.
        let confirmed = app
            .dialog()
            .message(format!(
                "{} already exists. Replace it with the current DiagramDesigner Next document?",
                destination.display()
            ))
            .title("Replace existing DDNX file?")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::YesNo)
            .blocking_show();
        if !confirmed {
            let document = lock_document(&state)?;
            return Ok(cancelled_save_result(&document));
        }
    }

    let report = atomic_save(&destination, prepared.bytes()).map_err(map_atomic_save_error)?;

    let mut document = lock_document(&state)?;
    if !document.session.acknowledge_document_saved(prepared.key()) {
        return Err(CommandError::committed(
            "stale_save_completion",
            "The file was written, but the document session was replaced before the save completed.",
        ));
    }
    if intent.updates_persistent_path(has_existing_path) {
        document.path = Some(destination);
    }
    document.imported_dirty = false;
    document.recovered_dirty = false;

    Ok(SaveResultDto {
        cancelled: false,
        state: document_state_dto(&document),
        commit_mode: Some(match report.mode {
            CommitMode::Created => "created",
            CommitMode::Replaced => "replaced",
        }),
        durability: Some(match report.durability {
            DurabilityLevel::FileAndDirectorySynced => "file-and-directory-synced",
            DurabilityLevel::FileSyncedAndPlatformCommitFlushed => {
                "file-synced-platform-commit-flushed"
            }
        }),
        cleanup_warning: report.cleanup_warning,
    })
}

fn cancelled_save_result(document: &DesktopDocument) -> SaveResultDto {
    SaveResultDto {
        cancelled: true,
        state: document_state_dto(document),
        commit_mode: None,
        durability: None,
        cleanup_warning: None,
    }
}

'''
lib.write_text(text[:start] + new_save + text[end:], encoding='utf-8')
replace_once(lib, '            save_document,\n            recovery_status,\n', '            save_document,\n            save_as_document,\n            recovery_status,\n')

build = ROOT / 'apps/desktop/src-tauri/build.rs'
replace_once(build, '            "save_document",\n            "recovery_status",\n', '            "save_document",\n            "save_as_document",\n            "recovery_status",\n')

permissions = ROOT / 'apps/desktop/src-tauri/permissions/editor.toml'
needle = '''[[permission]]
identifier = "allow-save-document"
description = "Allows the main editor window to invoke the save_document application command."
commands.allow = ["save_document"]
'''
replacement = needle + '''
[[permission]]
identifier = "allow-save-as-document"
description = "Allows the main editor window to invoke the save_as_document application command."
commands.allow = ["save_as_document"]
'''
replace_once(permissions, needle, replacement)

capability = ROOT / 'apps/desktop/src-tauri/capabilities/main-editor.json'
replace_once(capability, '    "allow-save-document",\n    "allow-recovery-status",\n', '    "allow-save-document",\n    "allow-save-as-document",\n    "allow-recovery-status",\n')

index = ROOT / 'apps/desktop/ui/index.html'
replace_once(
    index,
    '          <button id="save-document" class="primary" type="button">Save</button>\n',
    '          <button id="save-document" class="primary" type="button">Save</button>\n          <button id="save-as-document" type="button" title="Save as a new DDNX file (Ctrl/Cmd+Shift+S)">Save As…</button>\n',
)
replace_once(
    index,
    'Ctrl/Cmd+C, Ctrl/Cmd+V and Ctrl/Cmd+D copy, paste and duplicate outside text editing.',
    'Ctrl/Cmd+C, Ctrl/Cmd+V and Ctrl/Cmd+D copy, paste and duplicate outside text editing; Ctrl/Cmd+Shift+S opens Save As outside text editing.',
)

shortcuts = ROOT / 'apps/desktop/ui/editor-interaction/app-shortcuts.mjs'
replace_once(
    shortcuts,
    '''  if (textEditing) {
    return null;
  }
  if (command && normalized === 'z') {
''',
    '''  if (textEditing) {
    return null;
  }
  if (command && normalized === 's' && shiftKey) {
    return 'save-as';
  }
  if (command && normalized === 'z') {
''',
)

shortcut_tests = ROOT / 'web/editor-interaction/app-shortcuts.test.mjs'
replace_once(
    shortcut_tests,
    "  assert.equal(resolveApplicationShortcut({ key: 's', ctrlKey: true }, { textEditing: true }), 'save');\n",
    "  assert.equal(resolveApplicationShortcut({ key: 's', ctrlKey: true }, { textEditing: true }), 'save');\n  assert.equal(resolveApplicationShortcut({ key: 's', ctrlKey: true, shiftKey: true }, { textEditing: true }), null);\n",
)
insert_after = '''test('save is global while editor undo/delete stays out of text inputs', () => {
  assert.equal(resolveApplicationShortcut({ key: 's', ctrlKey: true }, { textEditing: true }), 'save');
  assert.equal(resolveApplicationShortcut({ key: 's', ctrlKey: true, shiftKey: true }, { textEditing: true }), null);
  assert.equal(resolveApplicationShortcut({ key: 'z', ctrlKey: true }, { textEditing: true }), null);
  assert.equal(resolveApplicationShortcut({ key: 'Delete' }, { textEditing: true }), null);
});
'''
replace_once(
    shortcut_tests,
    insert_after,
    insert_after + '''
test('save as resolves only outside text editing', () => {
  assert.equal(resolveApplicationShortcut({ key: 's', ctrlKey: true, shiftKey: true }), 'save-as');
  assert.equal(resolveApplicationShortcut({ key: 'S', metaKey: true, shiftKey: true }), 'save-as');
});
''',
)

app = ROOT / 'apps/desktop/ui/app.js'
replace_once(app, "  saveDocument: document.querySelector('#save-document'),\n", "  saveDocument: document.querySelector('#save-document'),\n  saveAsDocument: document.querySelector('#save-as-document'),\n")
replace_once(app, '  elements.saveDocument,\n  elements.undo,\n', '  elements.saveDocument,\n  elements.saveAsDocument,\n  elements.undo,\n')
old_save_fn = '''function saveCurrentDocument() {
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
'''
new_save_fn = '''function saveStatus(result, prefix = 'Saved') {
  const mode = result.commitMode === 'replaced' ? 'replaced atomically' : 'created atomically';
  return result.cleanupWarning
    ? `${prefix} (${mode}); temporary cleanup warning`
    : `${prefix} (${mode})`;
}

function saveCurrentDocument() {
  void runAction('save_document', undefined, (result) => saveStatus(result), { syncRecovery: true });
}

function saveAsCurrentDocument() {
  void runAction('save_as_document', undefined, (result) => saveStatus(result, 'Saved as'), {
    syncRecovery: true,
  });
}
'''
replace_once(app, old_save_fn, new_save_fn)
replace_once(app, 'elements.saveDocument.addEventListener(\'click\', saveCurrentDocument);\n', "elements.saveDocument.addEventListener('click', saveCurrentDocument);\nelements.saveAsDocument.addEventListener('click', saveAsCurrentDocument);\n")
replace_once(
    app,
    '''        if (shortcut === 'save') {
          saveCurrentDocument();
        } else if (shortcut === 'undo') {
''',
    '''        if (shortcut === 'save') {
          saveCurrentDocument();
        } else if (shortcut === 'save-as') {
          saveAsCurrentDocument();
        } else if (shortcut === 'undo') {
''',
)

print('Applied Save As and overwrite-confirmation product patch.')
