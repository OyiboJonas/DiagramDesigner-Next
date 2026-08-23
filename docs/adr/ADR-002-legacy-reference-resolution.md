# ADR-002: Resolve legacy index references in two passes

- Status: Accepted
- Date: 2026-08-19

## Context

Legacy Diagram Designer connectors persist relationships as numeric object indices within an owning object list plus a link-point index. Those indices are serialization positions, not stable identity. Resolving them while bytes are still being read would couple decoding order, object construction and graph identity.

## Decision

The legacy importer uses two passes:

1. decode every object payload and retain raw `ObjectIndex` / `LinkIndex` pairs;
2. after the complete owning list exists, validate and resolve those pairs to importer-stable identities.

The future Next domain never uses legacy list indices as element identity. Conversion emits stable IDs and typed endpoint/port references.

## Consequences

- Forward references are naturally supported.
- Dangling or invalid legacy references can be reported precisely.
- The byte codec stays deterministic and pointer-free.
- Group/nested-list ownership remains explicit.
- The future DDNX schema is not contaminated by legacy ordering semantics.
