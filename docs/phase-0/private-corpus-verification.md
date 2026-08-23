# Private / external legacy corpus verification

Private legacy diagrams do not need to be committed to GitHub in order to participate in repeatable migration regression testing.

`dd-migrate verify-corpus` reads a JSON manifest, verifies each source file SHA-256, runs the normal bounded inspection/decoding path, normalizes legacy text, converts through `legacy-migrate`, validates the `next-domain` artifact and calculates a deterministic converted-artifact fingerprint.

## Public/private boundary

The public repository contains only a generic manifest example. A real private manifest must live outside the repository (or in an ignored local path) and may contain private document names, local paths and reviewed fingerprints.

Do **not** commit a real private manifest, fixture, generated review output or private document fingerprint unless every field has been explicitly approved for publication.

## Manifest

Start from `fixtures/private-corpus.example.json` and create a private copy outside Git:

```json
{
  "entries": [
    {
      "name": "private-reference.ddd",
      "path": "/private/location/private-reference.ddd",
      "source_sha256": "<64-character-source-sha256>",
      "next_sha256": null
    }
  ]
}
```

`source_sha256` is mandatory. A mismatch stops the fixture before parsing.

`next_sha256` is optional. Once a conversion has been reviewed, it can be pinned in the **private** manifest so later semantic changes become explicit regression events.

## Optional text fingerprint

A private manifest may also pin the exact `TextNormalizationSummary`:

```json
{
  "entries": [
    {
      "name": "private-reference.ddd",
      "path": "/private/location/private-reference.ddd",
      "source_sha256": "<64-character-source-sha256>",
      "next_sha256": "<reviewed-next-json-sha256>",
      "text": {
        "entries": 123,
        "object_text_entries": 45,
        "decode_error_entries": 0,
        "markup_diagnostics": 0,
        "symbol_glyphs": 2,
        "action_tails": 1,
        "hint_tails": 0
      }
    }
  ]
}
```

The `text` block is optional. When present, every field is exact.

## Run

Direct verifier:

```text
cargo run -p dd-migrate -- verify-corpus <private-manifest.json>
```

Convenience harness:

```powershell
.\benchmarks\private-ddd\prepare-private-corpus.ps1 `
  -ManifestPath <private-manifest.json>
```

For a corpus requiring another explicit ANSI fallback:

```powershell
.\benchmarks\private-ddd\prepare-private-corpus.ps1 `
  -ManifestPath <private-manifest.json> `
  -FallbackEncoding windows-1250
```

Generated review output is written below the ignored `benchmark-results/private-ddd/` path by default. It is local review material, not public evidence.

## Security and repository policy

- Private binary files remain outside the Git repository.
- Private document names, paths and source fingerprints remain outside the public Git history.
- Generated private review output remains uncommitted unless explicitly approved.
- External fixture locations are read-only inputs to the verifier.
- `legacy-migrate` remains the sole translation boundary; private verification does not introduce another parser/converter.
- CI validates the generic harness with `-ValidateOnly`, which reads no private manifest or fixture.
- Public compatibility claims must be phrased at capability level unless supported by redistributable public fixtures.
