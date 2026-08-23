# ADR-019 renderer decision record

> Status: **not decided**. This file is the Phase-1 decision record template. Do not change the status to accepted/rejected until representative target-hardware evidence and the manual fidelity review are complete.

## Decision identity

- Source commit: `<40-character commit>`
- Decision date: `<ISO-8601 date>`
- Reviewer(s): `<name/role>`
- Combined target evidence session: `<benchmark-results/phase-1-target/target-...>`
- Combined manifest SHA-256: `<sha256>`
- GitHub Actions validation run for the same commit: `<run id / URL>`

The source commit used for the decision must match the source commit recorded in the combined evidence manifest. Evidence from another commit is supporting context only and cannot be substituted into the decision.

## Evidence eligibility

Record directly from `phase-1-target-evidence.json`:

- `workingTreeCleanAtStart`: `<true|false>`
- `eligibleForPhase1Decision`: `<true|false>`
- `diagnosticOnly`: `<true|false>`
- root `Cargo.lock` Git blob: `<40-character blob>`
- desktop `Cargo.lock` Git blob: `<40-character blob>`

Decision rule: the final Phase-1 renderer decision requires `eligibleForPhase1Decision = true` and `diagnosticOnly = false`.

## PreparedPage release conclusion

Record the representative 5k/20k release measurements from the combined archive.

| Scene | Rebuild p95 | Cache-hit p95 | Eviction rebuild | Conclusion |
| --- | ---: | ---: | ---: | --- |
| 5,000 | `<value>` | `<value>` | `<value>` | `<acceptable / investigate>` |
| 20,000 | `<value>` | `<value>` | `<value>` | `<acceptable / investigate>` |

PreparedPage architecture conclusion:

- [ ] immutable rebuild strategy accepted for Phase 1;
- [ ] incremental patching required before renderer promotion;
- [ ] measurement invalid/not representative — repeat target run.

Rationale: `<short evidence-based explanation>`

Do not justify incremental patching from hosted-CI/debug timing alone.

## ADR-019 native SVG performance

Record the mechanical verdict from the retained native Tauri/WebView2 report:

- Runtime: `<tauri-webview2>`
- Platform: `<windows>`
- Physical client area: `<width>x<height>`
- Monitor: `<name / dimensions>`
- 5k culled p95: `<value>`
- 20k culled p95: `<value>`
- Long Task requirement: `<pass/fail>`
- viewport-bounded DOM requirement: `<pass/fail>`
- mechanical performance verdict: `<performance_gate_pass|...>`

Performance conclusion: `<pass / fail / repeat measurement>`

A mechanical pass is necessary but not sufficient for renderer promotion.

## Fidelity evidence identity

Record from the nested fidelity archive:

- Fidelity manifest: `<relative path>`
- Fidelity manifest SHA-256: `<sha256>`
- SVG SHA-256: `<sha256>`
- Diagnostics SHA-256: `<sha256>`
- Candidate: `<render-svg>`
- Fidelity eligibility: `<true|false>`
- Automated manual-review status before review: `<not-reviewed-by-runner>`

Expected typed diagnostics present:

- [ ] `ConnectorMarkerDeferred`
- [ ] `UnsupportedPrimitive`

Unexpected diagnostics: `<none or list>`

## Manual correctness/fidelity review

Use `docs/architecture/adr-019-fidelity-review.md` as the review contract.

| Check | Result | Notes |
| --- | --- | --- |
| Master layer remains behind page-local layer | `<correct / blocking / out-of-scope>` | `<notes>` |
| Rounded rectangle and rotations | `<...>` | `<notes>` |
| Ellipse stroke/fill/alpha | `<...>` | `<notes>` |
| Horizontal and vertical gradients | `<...>` | `<notes>` |
| Unicode/XML-sensitive text and page fields | `<...>` | `<notes>` |
| Supported connector dash styles | `<...>` | `<notes>` |
| Four rotated page-edge sentinels remain visible/clipped correctly | `<...>` | `<notes>` |
| Deferred marker remains explicit diagnostic | `<...>` | `<notes>` |
| Unsupported polygon remains explicit diagnostic | `<...>` | `<notes>` |
| Review at 100% and representative zoom levels | `<...>` | `<notes>` |

Blocking fidelity defects: `<none or list>`

Accepted typed approximations/deferred semantics: `<none or list with rationale>`

Manual fidelity conclusion: `<acceptable / blocking / repeat review>`

## Renderer decision

Select exactly one only after all sections above are complete:

- [ ] **SVG selected as Phase-1 production renderer.**
- [ ] **SVG rejected for Phase 1; evaluate Canvas2D fallback.**
- [ ] **SVG rejected for Phase 1; evaluate WebGL fallback.**
- [ ] **SVG rejected for Phase 1; evaluate Qt/native fallback.**
- [ ] **No decision — evidence must be repeated or architecture work remains open.**

Final rationale: `<evidence-based explanation>`

## Promotion consequences

If SVG is selected:

- production primitive rendering may be promoted/hardened through the existing renderer abstraction;
- the renderer-independent `render-plan`, editor geometry and command contracts remain authoritative;
- deferred/approximated semantics remain explicit until separately implemented;
- do not move SVG-specific state into `next-domain` or editor history.

If SVG is rejected:

- keep the current abstraction and test/evidence contracts;
- implement the smallest viable fallback adapter without page-specific forks;
- repeat equivalent correctness/performance evidence before selecting the fallback.

## Phase-1 closure

Phase 1 may be marked complete only when:

- [ ] combined target evidence is decision-eligible;
- [ ] PreparedPage conclusion is recorded;
- [ ] native renderer performance conclusion is recorded;
- [ ] manual fidelity review is complete with no unresolved blocking defect;
- [ ] final renderer decision is recorded above;
- [ ] production renderer follow-up is clearly assigned or implemented as required by the decision;
- [ ] PR #12 and issue #11 are updated to the same source/evidence checkpoint.
