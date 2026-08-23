# Public repository policy

DiagramDesigner Next is intended to be usable as a public source repository while still supporting compatibility testing against private legacy documents.

## Never commit by default

The public repository must not contain private/company fixture binaries or identifying metadata derived from them, including:

- private document names;
- local/private filesystem paths;
- source-file fingerprints or private converted-artifact fingerprints;
- decoded private document text;
- screenshots/previews of private documents;
- document-specific structural statistics that could act as a fingerprint;
- generated private corpus manifests or review outputs;
- credentials, tokens, `.env` files or private keys.

Exceptions require explicit publication approval for the exact material.

## Private compatibility testing

Private fixtures are consumed only through an external manifest passed to:

```powershell
.\benchmarks\private-ddd\prepare-private-corpus.ps1 `
  -ManifestPath <private-manifest.json>
```

The manifest and fixture store remain outside Git. Generated output is written below `benchmark-results/`, which is ignored.

CI may run the private-corpus harness only with `-ValidateOnly`; this validates tooling without opening a private manifest or fixture.

## Public fixtures

Redistributable public fixtures may be pinned with exact hashes and regression expectations when their license/provenance permits redistribution or automated retrieval. The upstream Diagram Designer DDT compatibility corpus is checked out from its public upstream repository at a pinned revision rather than being treated as DiagramDesigner Next-owned content.

## Preflight

Run the built-in tracked-tree scan before export:

```powershell
.\scripts\public-preflight.ps1
```

For organization-specific/private identifiers, create a local text file outside Git with one forbidden literal per line (for example internal names, private fixture names and private fingerprints) and run:

```powershell
.\scripts\public-preflight.ps1 `
  -ForbiddenTermsFile <local-private-terms.txt>
```

The forbidden-terms file itself must never be committed.

## History-free export

The existing private repository/history is not a publication source. Export only the cleaned tracked tree. For the final export, pass the same local forbidden-terms file so the sensitive scan is part of the export gate:

```powershell
.\scripts\export-public-snapshot.ps1 `
  -ForbiddenTermsFile <local-private-terms.txt>
```

The exporter requires a clean working tree, runs the preflight, and creates a `git archive` ZIP outside the repository. The ZIP contains tracked `HEAD` files only and no `.git` history, issues, pull requests or branch metadata.

The ZIP should be used to seed a **new repository with a fresh initial commit**. Do not push the existing private repository history to the public remote.

## Before a public release

A release/publication review should verify:

1. no private fixture/material has entered the tracked tree;
2. no credentials or environment files are tracked;
3. generated benchmark/private-corpus output is absent;
4. license and third-party notices are present;
5. generic and local forbidden-term preflight checks pass;
6. the public CI suite passes;
7. publication uses the history-free snapshot and a fresh Git history;
8. the release is created from a clean source tree.
