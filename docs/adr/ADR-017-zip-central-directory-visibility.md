# ADR-017 — ZIP central-directory visibility invariant

- Status: Accepted for DDNX v1 security boundary
- Date: 2026-08-19
- Scope: `ddnx` archive reader

## Context

DDNX ZIP files are untrusted input. The v1 reader already rejected duplicate logical names by inserting every `ZipArchive::by_index(...)` name into its own map and failing on a duplicate.

A foreign ZIP fixture created independently with Python's standard-library ZIP writer exposed a library-level edge case: the pinned Rust `zip` 7.0.0 reader stores central-directory files in an `IndexMap` keyed by decoded filename. When two central-directory entries use the same filename, the later insertion replaces the earlier one. `ZipArchive::len()` and index iteration can therefore expose fewer files than the ZIP central directory actually declares.

A duplicate check performed only over the library-visible entries is insufficient because the hidden/collapsed entry never reaches that loop.

## Decision

DDNX v1 validates **central-directory visibility** before parsing `manifest.json`.

The reader independently obtains the entry count declared by the ZIP end-of-central-directory metadata and compares it with the number of entries exposed by `ZipArchive`.

```text
raw ZIP bytes
  → bounded EOCD / ZIP64 EOCD entry-count read
  → enforce package entry-count limit
  → construct ZipArchive
  → compare declared count with visible count
  → only if equal: normal path/name/type/size/overlap/hash checks
```

If the counts differ, the package is rejected with `ArchiveEntryCountMismatch`. No manifest or document payload is interpreted.

The existing logical-name duplicate check remains in place as a second layer for any duplicate/collision form the library does expose distinctly.

## EOCD handling

The independent count reader is intentionally narrow. It does not become a second ZIP implementation.

It reads only the bounded metadata required for the security invariant:

- classic end-of-central-directory signature and comment length;
- disk numbers and entry counts;
- ZIP64 sentinel detection;
- ZIP64 locator and ZIP64 end-of-central-directory entry counts when required.

Multi-disk archives are rejected. Truncated, inconsistent or out-of-bounds central-directory metadata is rejected fail-closed.

All normal entry parsing, compression/type inspection, overlap detection and payload reads remain delegated to the pinned `zip` library and the existing DDNX checks.

## Why compare counts instead of reimplementing filename parsing

The security property we need is stronger and simpler than reconstructing every ZIP filename ourselves:

> Every central-directory entry in the source archive must be visible to the library layer on which all later DDNX validation operates.

If a library collapses two names, ignores an entry or otherwise exposes a different file count, that invariant is already broken and the archive is rejected.

This avoids building a parallel filename-decoding/canonicalization implementation that could disagree with the actual ZIP reader.

## ZIP64

`PackageLimits::max_entries` currently exceeds the classic 65,535-entry field, so valid bounded DDNX input can theoretically require ZIP64 entry counts. The visibility precheck therefore understands ZIP64 EOCD counts rather than banning ZIP64 as an ad-hoc workaround.

The normal DDNX package limits still apply after the count is obtained.

## Tests

The permanent adversarial suite contains two independent duplicate-related checks:

1. the production Rust `ZipWriter` refuses creation of a duplicate logical name;
2. a fixed foreign ZIP byte fixture generated outside the Rust `zip` crate declares two `manifest.json` entries. The pinned library exposes one; DDNX rejects the archive before manifest parsing because `declared = 2` and `visible = 1`.

This fixture protects the visibility invariant across future ZIP-library upgrades. If a future library exposes both duplicate entries, the existing logical-name duplicate check must reject them instead; the archive remains invalid either way.

## Consequences

- library-internal name deduplication cannot silently become DDNX last-wins behavior;
- the DDNX reader remains fail-closed for malformed/hidden central-directory entries;
- ZIP64 entry counts remain supported within normal DDNX limits;
- future upgrades of the ZIP dependency must keep both the visibility and duplicate-name tests green.
