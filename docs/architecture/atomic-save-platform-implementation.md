# Atomic save platform implementation

This note records the first concrete implementation of ADR-015 in `crates/platform-fs`.
The crate consumes complete immutable bytes. It does not know how DDNX is encoded and it does not decide whether an overwrite is acceptable to the user.

## Global commit sequence

1. Inspect the destination without following a destination symlink.
2. Create a unique sibling temporary file with `create_new` semantics.
3. Write the complete payload, flush it and call `sync_all` on the temporary file.
4. Commit the sibling using the platform path below.
5. Perform the strongest post-commit directory/rename durability operation currently exposed by the adapter.
6. Return success only after the selected durability step succeeds.

The adapter never uses `delete(destination) -> rename(temp, destination)`.
Failures before the commit point report `committed = false`. A post-commit durability failure reports `committed = true`, allowing the application to explain that the destination may already contain the new data while still refusing to mark the editor state as durably saved.

## Unix-like systems

### New destination

A new destination is committed with a hard link from the already-synced sibling temporary to the final destination name. Creating the hard link is an atomic create-if-absent operation: it cannot silently replace a destination that appeared between the initial inspection and commit.

After the destination link exists, the temporary link is removed. Failure to remove that second name is a cleanup diagnostic and never triggers destructive rollback of the valid destination.

This path is suitable for the local filesystems targeted by the project such as ext4 and APFS. Filesystems that do not support hard links can reject the operation; the adapter reports that limitation instead of falling back to a potentially destructive or racy sequence.

### Existing destination

An existing regular file is replaced with the native same-filesystem `rename` operation. Because the temporary is always a sibling, the adapter never intentionally crosses a mount or volume boundary.

### Durability

After the commit, the parent directory is opened and `sync_all` is called. The success report is therefore `FileAndDirectorySynced`.

## Windows

The Windows adapter uses `MoveFileExW` directly from `Kernel32`.

- new destination: `MOVEFILE_WRITE_THROUGH` without `MOVEFILE_REPLACE_EXISTING`;
- existing destination: `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`.

The replacement path is therefore a native same-volume rename/replace operation and never emulates replacement by deleting the old document first. The temporary file is synchronized before the call.

Windows does not expose the POSIX parent-directory `fsync` model through `std::fs`. The adapter therefore reports the narrower `FileSyncedAndPlatformCommitFlushed` durability level rather than claiming Unix-style directory durability.

A Windows CI job runs the same new/replace/failure-preservation tests against the native implementation.

## Cleanup

`cleanup_stale_siblings` only considers regular files whose names match the exact temporary prefix generated for one destination and whose modification age exceeds the caller-supplied threshold. Unrelated `.tmp` files and similarly named files are deliberately ignored.

The normal save path also attempts best-effort cleanup of a pre-commit temporary when writing, syncing or committing fails.

## What this does not guarantee

Atomic local rename/link primitives cannot by themselves guarantee equivalent behavior on every network share, FUSE filesystem, cloud-sync provider or storage appliance. The adapter reports the local operation that succeeded; it does not claim end-to-end durability through remote caching or synchronization layers.

Atomic replacement also does not prevent two editor processes from overwriting each other's valid versions. Source-file fingerprinting and external-modification conflict handling remain an application-session responsibility above this primitive, as required by ADR-015.

Recovery/autosave files are separate application policy. They may reuse this adapter, but recovery persistence must remain distinct from the primary user-visible saved marker.
