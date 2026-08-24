# Desktop legacy import contract

The desktop **Open** workflow accepts native `.ddnx` packages and legacy DiagramDesigner `.ddd` / `.ddt` sources.

## Persistence boundary

Native `.ddnx` opens keep their selected path as the persistence target. Legacy files are different: they are read-only import sources. The desktop stores their path only as transient provenance/display state and deliberately leaves the editable document persistence path empty. The existing first-save flow therefore requires a new `.ddnx` destination and cannot overwrite the legacy source.

An imported copy owns an explicit desktop `imported_dirty` state because the newly-created editor session itself correctly considers its initial history state clean. That desktop state keeps the imported copy visibly unsaved and eligible for recovery checkpoints until a `.ddnx` save succeeds.

## DDD and DDT mapping

`.ddd` migration must produce a Next document artifact. `.ddt` migration must produce a Next template palette; the desktop then explicitly materializes that palette as a one-page editable document. Palette size, scene, styles, assets and import metadata are preserved. Fresh document/page/layer IDs are created and the normal desktop document defaults are applied.

The extension selects the expected legacy artifact family. A mismatched payload is rejected instead of being silently reinterpreted.

## Recovery and provenance

The original legacy source path is never serialized into the Next document. This avoids persisting machine-local filesystem paths. It is retained in the live desktop state for provenance/display while the session is running. Recovery snapshots remain normal `.ddnx` document snapshots; restoring one is therefore intentionally detached from the original source path and follows the existing recovered-copy Save As contract.
