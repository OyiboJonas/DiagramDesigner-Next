# ADR-021 — Recovery checkpoints and user-visible save state

- Status: Accepted for Phase 1
- Date: 2026-08-21
- Scope: editor runtime, autosave/recovery, history state, platform persistence boundary

## Context

DiagramDesigner Next already separates persistent editor commands from transient interaction and tracks the user-visible dirty state through `HistoryStateId`. A successful explicit Save operation can therefore mark one history state as saved without serializing undo/redo history into DDNX.

Crash/session recovery has a different purpose. It should periodically preserve the current native document state so work can be recovered after a process, OS or device failure. Treating that recovery write as a normal Save would be incorrect because it would:

- clear the dirty indicator even though the user's chosen document was not saved;
- change the meaning of undo/redo history bookkeeping;
- couple editor history to timer/lifecycle events;
- make asynchronous recovery writes capable of acknowledging the wrong document state;
- push filesystem and scheduling concerns back into `editor-core`.

Recovery also has an asynchronous identity problem. A recovery write may finish after additional edits have been committed, after Undo/Redo changes the current state, or even after another document has replaced the current editor session. `HistoryStateId` alone is intentionally session-local and is therefore not sufficient to identify an outstanding write across session replacement.

## Decision

### Recovery is external persistence, not an editor command

Creating, writing or deleting a recovery checkpoint never creates an `EditCommand`, never creates an undo/redo entry and never calls `mark_saved()`.

The authoritative user-visible dirty state remains:

`current HistoryStateId != saved HistoryStateId`.

Only a successful explicit document save may advance the saved marker.

### `editor-runtime` owns recovery planning

Recovery coordination belongs in `editor-runtime`, the existing application composition boundary between editor state and renderer-derived/application state.

`editor-core` remains unaware of autosave timers, filesystem paths, DDNX package writes and recovery-file lifecycle.

The runtime exposes a renderer- and platform-independent recovery plan with three outcomes:

- `None` — no platform action is currently needed;
- `Write(snapshot)` — persist the supplied immutable native-document snapshot as the current recovery checkpoint;
- `Remove(key)` — remove the exact acknowledged recovery checkpoint because it is no longer required.

The runtime does not perform filesystem I/O.

### Recovery snapshots contain native document state only

A recovery snapshot contains a cloned `NextArtifact::Document` representing one persistent editor history state.

It does not contain:

- undo/redo stacks;
- transient selection;
- pan/zoom state;
- renderer prepared-page caches;
- desktop-shell window state.

The platform/application layer may serialize the snapshot using the existing DDNX codec and persist it through the same crash-safe filesystem principles defined by ADR-015.

### Recovery identity combines session generation and history state

Each recovery checkpoint is identified by:

`RecoveryCheckpointKey = (session_generation, HistoryStateId)`.

`session_generation` changes whenever the complete `EditorSession` is replaced. This prevents a delayed write acknowledgement from an older document from being accepted as recovery state for the newly opened document, even when both sessions contain the same numeric history-state value.

### Same-session stale acknowledgements describe disk truth

If a recovery write for history state A completes after the editor has already advanced to history state B in the same session, its acknowledgement is accepted as the state that is actually durable on disk.

The next `recovery_plan()` call then compares the acknowledged checkpoint with the current history state and immediately requests a new snapshot for B.

This avoids pretending that a stale write never happened while still guaranteeing that it cannot mask a newer dirty state.

### Undo/Redo participate through history identity only

Undo and Redo do not receive recovery-specific branches.

Because they restore exact `HistoryStateId` values, recovery planning naturally observes the restored state. If the on-disk recovery checkpoint represents another history state, the runtime requests replacement with the exact current state.

Recovery therefore composes with history without becoming part of history.

### Clean user-visible state removes recovery data

Once the current editor state is no longer dirty and a recovery checkpoint is known to exist, the runtime requests removal of that exact checkpoint.

This covers explicit Save and also history navigation back to the saved state.

Removal acknowledgement is key-specific. A stale deletion acknowledgement cannot clear a newer checkpoint.

### Session replacement keeps old recovery knowledge until cleanup

Replacing the complete editor session increments the session generation and clears renderer-derived snapshots as before.

An already acknowledged recovery key from the previous session remains known until the platform layer removes or supersedes it. This lets the application clean abandoned recovery data without allowing old write acknowledgements to contaminate the new session.

### Filesystem mechanics stay behind the platform boundary

ADR-021 defines when recovery data should be written or removed. It does not define how bytes are committed to disk.

Atomic temp-file creation, flush/commit behavior, destination replacement and platform-specific durability remain the responsibility of the global filesystem adapter described by ADR-015 and issue #10.

The desktop shell will later decide:

- recovery storage location and naming;
- autosave cadence and lifecycle triggers;
- DDNX encoding/verification before commit;
- startup discovery and recovery UX;
- retention/cleanup policy for abandoned checkpoints.

## Consequences

### Positive

- autosave can never make an unsaved document appear saved;
- recovery writes do not pollute undo/redo history;
- editor-core remains free of filesystem, timer and desktop-lifecycle dependencies;
- delayed writes are deterministic and cannot acknowledge a replaced document session;
- Undo/Redo automatically cooperate with recovery through existing persistent-state identity;
- the later Tauri shell receives a small explicit Write/Remove/None contract rather than mutable editor internals;
- DDNX and the filesystem adapter retain their existing responsibilities.

### Cost

- the application layer must acknowledge successful recovery writes/removals explicitly;
- one complete native document snapshot is cloned when a new recovery write is requested;
- session generation and persisted-checkpoint bookkeeping must survive long enough to reject stale asynchronous completions;
- startup recovery discovery remains a separate desktop-shell concern.

## Phase-1 implementation

PR #12 implements the runtime contract in `editor-runtime` with dedicated tests covering:

- recovery checkpoints leaving saved state and undo history unchanged;
- newer edits superseding stale same-session recovery writes;
- exact recovery behavior across Undo/Redo history restoration;
- session replacement rejecting stale write acknowledgements;
- cleanup of acknowledged recovery data after the document becomes clean.

The runtime remains filesystem-independent; issue #10 is still the implementation boundary for atomic platform persistence.

## Follow-on requirements

- connect `RecoveryPlan::Write` to verified DDNX encoding and the platform filesystem adapter;
- define a desktop autosave cadence and lifecycle/suspend trigger policy;
- define recovery-file naming/location without granting broad filesystem capability to the webview;
- add startup discovery and explicit restore/discard UX;
- test abrupt process termination and stale-recovery cleanup on Windows;
- keep recovery checkpoints out of user-visible recent-file/save semantics unless explicitly restored.

Tracks #11 and implementation PR #12. Filesystem durability follow-up: #10 / ADR-015.
