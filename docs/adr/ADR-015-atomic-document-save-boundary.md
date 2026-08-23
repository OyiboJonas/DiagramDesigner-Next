# ADR-015 — Atomic document save boundary

- Status: Accepted architecture policy; platform implementation follows with desktop shell
- Date: 2026-08-19
- Scope: document save pipeline, filesystem/platform adapter, DDNX integration

## Context

The `ddnx` crate owns package serialization and validation. It deliberately does not own native filesystem semantics.

The migration CLI currently supports creation of a **new** output file by writing a unique sibling temporary file, syncing it and renaming it into place. It refuses to overwrite an existing target. That is sufficient for the Phase-0 conversion tool but not for a real editor, where Save must safely replace an existing document.

A delete-then-rename sequence is unacceptable: a crash or power loss between those operations can remove the last valid document. Cross-volume temporary files are also unacceptable because rename may cease to be atomic.

## Decision

### Separation of responsibilities

`ddnx` produces and validates complete package bytes. It has no API that silently overwrites paths.

The application/platform filesystem adapter owns committing those bytes to a user-selected path.

```text
NextArtifact
  → ddnx serialize + self-verify
  → complete immutable package bytes
  → platform atomic-save adapter
  → durable user file
```

### Save to a new path

For a destination that does not exist:

1. create a uniquely named temporary **sibling** in the destination directory using create-new semantics;
2. write the complete package bytes;
3. flush/sync the temporary file according to the platform durability capability;
4. atomically rename the sibling to the destination;
5. sync the containing directory where the platform exposes a meaningful directory-durability primitive;
6. report success only after the commit steps complete.

The sibling requirement keeps the temporary file on the same filesystem/volume as the destination.

### Replace an existing path

For Save over an existing regular file:

1. write and sync a unique sibling temporary file exactly as above;
2. use the platform's atomic **replace** primitive for the existing destination;
3. never implement replacement as `delete(destination)` followed by `rename(temp, destination)`;
4. retain the old destination until the replace operation itself succeeds;
5. perform the platform-appropriate containing-directory durability step after replacement;
6. report any durability limitation or error rather than claiming a stronger guarantee than the platform provides.

The Windows adapter must use a native replace/rename operation with same-volume atomic replacement semantics rather than emulating POSIX behavior through deletion. The Unix adapter may use its native same-filesystem rename/replace semantics and directory sync behavior. Exact APIs belong in the platform crate and require platform-specific tests.

### Failure behavior

Before the final commit/replace succeeds:

- the existing destination must remain untouched;
- a failed write/sync must never truncate the destination;
- best-effort cleanup may remove the temporary sibling;
- inability to remove a temporary file is a cleanup diagnostic, not permission to modify the destination.

After a successful replace but failed post-commit durability sync, the adapter returns a durability error/warning according to the application policy; it must not attempt a destructive rollback without a separately designed journal/backup mechanism.

### Concurrency / stale-file protection

Atomic replacement prevents torn writes but does not solve two editors overwriting each other's changes. The future document session layer must maintain a source-file identity/fingerprint and check for external modification before replacing an existing file. Conflict handling is an application concern layered above this atomic-save primitive.

### Backups and recovery

Backup copies and autosave/recovery files are separate features. They may use the same atomic-save adapter but must not change the fundamental commit algorithm for the primary document.

## Consequences

### Positive

- DDNX remains testable without platform filesystem side effects;
- existing user documents survive failures before the atomic commit point;
- temporary files are guaranteed to be on the destination filesystem;
- Windows and Unix behavior can be implemented correctly without scattering platform conditionals through editor code;
- autosave, Save As and normal Save can share one global commit component.

### Cost

- the desktop/platform layer needs OS-specific implementation and tests;
- durability guarantees must be represented honestly because filesystem and OS semantics differ;
- external-modification detection remains a separate session-layer concern.

## Phase-0 implication

`dd-migrate convert --output` continues to refuse overwrite and is not promoted into the editor's Save implementation. The future desktop filesystem adapter implements this ADR and consumes already self-verified DDNX bytes.

Issue #8 can treat the **policy** as defined while keeping platform implementation open until the desktop shell/platform crate exists.
