# ADR-013 — DDNX version and migration policy

- Status: Accepted for pre-v1 format freeze
- Date: 2026-08-19
- Scope: `ddnx`, future document-open/save pipeline, migration tooling

## Context

DDNX deliberately has three independent version axes:

1. `PACKAGE_VERSION` — ZIP/package contract and entry semantics;
2. `DOCUMENT_VERSION` — DDNX-specific `document.json` persistence projection;
3. `next-domain::SCHEMA_VERSION` — renderer-independent semantic domain schema.

Treating those values as one version would couple container changes, persistence-shape changes and domain evolution. Treating unknown versions as if they were current would create silent data-loss risk.

## Decision

### Current reader/writer

The pre-v1 reader accepts only the exact currently implemented package, document and Next-schema versions. The writer emits only those current versions.

An unknown older or newer version is rejected explicitly. The reader must not attempt best-effort deserialization against the current structs.

### Future backward compatibility

When DDNX version 2 or later exists, backward compatibility is implemented as explicit monotonic migration steps:

```text
verified older package envelope
  → version-specific older projection
  → migrate N → N+1
  → ...
  → current projection/domain
  → normal NextArtifact validation
```

Every supported transition is a named, testable step. Skipping intermediate semantic migrations is not allowed unless that direct transition itself is an explicit tested migration.

### Safety order

Before version-specific payload hydration, the package layer may inspect only the bounded envelope information required to identify the format/version and enforce archive safety. Unknown-version payloads are not hydrated into current domain structs.

Future legacy-version readers must retain the same archive safety properties as the current reader: bounded entry counts and sizes, normalized paths, no filesystem extraction, declared-entry validation and integrity verification appropriate to that source version.

### Newer files

A reader must reject package/document/domain versions newer than it understands. It may report the versions found so the application can explain that a newer DiagramDesigner Next version is required.

It must not silently drop unknown fields or coerce a newer artifact into the current schema.

### Save behavior

Opening an older supported DDNX version migrates it **in memory** to the current domain. The original source file is not rewritten merely by opening it.

A normal save writes only the current package/document/domain versions. Overwriting/replacing the source is an application/filesystem decision and follows the atomic-save policy, not the migration codec.

### Downgrade/export

The normal writer does not emit older DDNX versions. If a future product requirement needs downgrade/export, it requires a separate explicit exporter with its own loss report. A version number parameter on the ordinary writer is not sufficient.

## Consequences

### Positive

- container, persistence and semantic evolution remain independent;
- unsupported newer files fail safely instead of losing data;
- backward compatibility is reviewable and testable one migration at a time;
- normal editor/domain code only sees the current `next-domain` model;
- opening an older file is non-destructive until the user saves.

### Cost

- supporting an old DDNX version requires retaining its version-specific projection/migrator or an equivalent audited migration fixture;
- version migrations add corpus and golden-test maintenance.

## Current implementation contract

`Manifest::validate()` rejects any `package_version`, `document_version` or `next_schema_version` that differs from the current constants. Hydration separately cross-checks document projection and manifest versions.

This strict behavior is intentional for v1 preparation. Future backward compatibility must extend the reader through explicit version dispatch/migrators rather than weakening these equality checks into permissive parsing.

## Required tests for every future version

A new supported version must add tests proving:

1. current writer emits only the new current version;
2. known previous version migrates deterministically to current domain;
3. unsupported newer versions are rejected without hydration;
4. malformed old-version packages remain bounded/untrusted input;
5. migration preserves IDs/assets/text/relationships or reports any deliberate loss;
6. migrated artifacts pass normal `NextArtifact::validate()`;
7. save-after-migration produces a valid current DDNX package.
