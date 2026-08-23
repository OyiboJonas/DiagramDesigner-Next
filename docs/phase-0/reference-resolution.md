# Legacy reference resolution

Diagram Designer connector endpoints persist object relationships as owner-list indices plus link-point indices. DiagramDesigner Next must not reuse those indices as document identity.

## Pass 1 — byte decode

Each owning object list is decoded in serialized order. Connector endpoints retain the exact legacy pair:

```text
object_index: i32
link_index: Option<u16>
```

No pointer, UI object or Next-domain reference is created while bytes are still being read.

## Pass 2 — materialized importer graph

After a complete owner list exists, `resolve_container_reference_graph` performs the second pass:

1. assign deterministic importer-local IDs such as `page/0/layer/0/object/17`;
2. recurse into groups with their own owner-list paths;
3. validate every non-negative `object_index` against that exact owner list;
4. validate `link_index` against the target object's source-defined link-point count;
5. materialize a `ResolvedEndpoint` containing the stable source ID and optional typed target;
6. retain the original `raw_object_index` and `raw_link_index` alongside the resolved target for traceability;
7. report dangling/out-of-range references explicitly rather than silently disconnecting them.

The compact `dd-migrate inspect` path exposes a `ReferenceResolutionSummary`; the full resolved graph remains available to the migration layer as `ResolvedReferenceGraph`.

## Real-file validation policy

The resolver has been exercised with private real-world DDD references in addition to public/synthetic fixtures. Identifying private fixture names, fingerprints and document-specific relationship counts are intentionally not published. Private regression evidence is replayed through the external manifest mechanism described in `private-corpus-verification.md`.

Public compatibility claims are therefore limited to the implemented validation behavior unless a redistributable fixture supports a more specific claim.

## Next-domain boundary

The deterministic path IDs are importer identities only. They are not intended as permanent DDNX IDs. The Next-domain conversion stage creates stable document element/port IDs and translates resolved legacy endpoints to those IDs.

This separation preserves legacy semantics while preventing serialized list position from leaking into the new editor architecture.
