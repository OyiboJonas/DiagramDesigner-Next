# ADR-001: Greenfield core with explicit legacy boundary

- Status: Accepted
- Date: 2026-08-18

## Context

Diagram Designer is a Delphi 7 / VCL application whose document model, rendering, editing behavior and persistence are tightly coupled. DiagramDesigner Next must preserve user documents without inheriting the legacy runtime architecture.

## Decision

DiagramDesigner Next is a greenfield implementation.

The original `meesoft/DiagramDesigner` codebase remains a behavioral and file-format reference. Legacy `.ddd` and `.ddt` support lives behind a dedicated read-only compatibility boundary (`crates/legacy-ddd`). The compatibility crate decodes into explicit intermediate structures and must not depend on editor UI, renderer or platform code.

New documents will eventually use the versioned DDNX format. A legacy writer is not part of the initial architecture.

## Consequences

- Legacy parsing can be fuzzed and hardened independently.
- The Next domain model is free to use stable IDs and typed data.
- Compatibility claims must be backed by golden files, not assumptions from source inspection alone.
- Any uncertain binary-layout detail is documented and verified against real files before being treated as stable.
