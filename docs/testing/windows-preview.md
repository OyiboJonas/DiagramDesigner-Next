# DiagramDesigner Next — Windows preview test guide

This preview is an unsigned development build. It is intended for functional testing only; it is not a production installer.

## Start

1. Extract the GitHub Actions artifact to a normal local folder.
2. Start `DiagramDesigner-Next-Preview.exe`.
3. Windows may show a SmartScreen warning because the preview is not code-signed. Only continue if the artifact came from this repository's `Windows desktop preview` workflow.
4. The application uses the Windows WebView2 runtime. Current Windows 10/11 installations normally already provide it.

## Primary legacy-import test

1. Click **Open…**.
2. Select a legacy DiagramDesigner `.ddd` file.
3. Verify that the document appears on the canvas and that the UI identifies it as an imported copy requiring a `.ddnx` save.
4. Select one or more visible elements and move them.
5. Test **Grid**, **Snap**, **Undo** and **Redo**.
6. Click **Save** and verify that a new `.ddnx` destination is requested.
7. Confirm that the original `.ddd` file was not modified.
8. Close/reopen the saved `.ddnx` and verify that the edited result is still present.

## DDT palette test

1. Click **Open…** and select a legacy `.ddt` palette.
2. Verify that it opens as a one-page editable Next document rather than as a separate palette editor.
3. Inspect several objects and, where possible, move one object and undo/redo the move.
4. Save to a new `.ddnx` file and confirm that the source `.ddt` remains unchanged.

## Native DDNX regression

1. Open an existing `.ddnx` file.
2. Verify that it is shown as a normal native document, not as an imported copy.
3. Make a small move, save, reopen and verify persistence.

## Please record for failures

For a reproducible failure, note:

- source file type (`.ddd`, `.ddt` or `.ddnx`);
- visible error/status message;
- which object or interaction caused it;
- whether the source file still opens in classic DiagramDesigner;
- the `source_commit` and `desktop_cargo_lock_blob` from `BUILD.txt`.

Do not add private/company `.ddd` or `.ddt` files to the public repository when reporting an issue.
