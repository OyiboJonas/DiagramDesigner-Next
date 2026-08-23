# ADR-019 renderer decision record

> Status: **SVG selected for Phase 1**. The representative Windows target evidence, PreparedPage conclusion and manual fidelity review are complete for the evidence source commit recorded below.

## Decision identity

- Source commit: `6c5595d62ddb905ed864203230c5a7786b36f860`
- Decision date: `2026-08-23`
- Reviewer: project maintainer (manual visual fidelity review)
- Combined target evidence session: `benchmark-results/phase-1-target/target-20260823-205022-6c5595d62ddb`
- Combined manifest: `phase-1-target-evidence.json` (retained locally with the verified target archive; its SHA-256 was not separately transcribed into the public review transcript)
- Evidence source tree: `f788f8757678d8ec3cefeb7e7283a34540451fc8`
- GitHub Actions validation: PR #7 head `b9557cd55082e49265e1c8f8e7ed8e419d93f5b6` has the exact same Git tree `f788f8757678d8ec3cefeb7e7283a34540451fc8` as the measured squash-merge source commit. All three PR validations passed: review viewer run `32657719131`, Windows native toolchain run `32657719154`, Rust compatibility lab run `32657719197`.

The retained evidence archive remains authoritative for raw machine/environment fields and per-sample timings. The public record intentionally stores the decision-relevant results and hashes rather than machine-specific identifying metadata.

## Evidence eligibility

Recorded from the verified combined target archive:

- `workingTreeCleanAtStart`: `true`
- `eligibleForPhase1Decision`: `true`
- `diagnosticOnly`: `false`
- root `Cargo.lock` Git blob: `02382480ada113134ca385aefec721badf344e92`
- desktop `Cargo.lock` Git blob: `a120ba1007d67b785517e12d6054f6ee0a1ab6a9`
- PreparedPage evidence SHA-256: `70298afcd51cf804e5067d6282e762f3f769a484ccdb9a4ff559c2217f3c5d7c`
- ADR-019 native renderer evidence SHA-256: `66f16ed1b88020b56eee4b92c1b71fa54a3f740298f93ac8c9c3a2b7c931b131`
- Fidelity manifest SHA-256: `5b7b065ee8d540e93afa18f5da9de206b084e7458488670f7768582d817aac82`

The final Phase-1 decision therefore satisfies the required decision-eligible/non-diagnostic source rule.

## PreparedPage release conclusion

Representative release measurements from the combined archive:

| Scene | Rebuild p95 | Cache-hit p95 | Eviction rebuild | Conclusion |
| --- | ---: | ---: | ---: | --- |
| 5,000 | `2001 us` | `100 ns` | `1318 us` | acceptable |
| 20,000 | `8967 us` | `100 ns` | `8980 us` | acceptable |

PreparedPage architecture conclusion:

- [x] immutable rebuild strategy accepted for Phase 1;
- [ ] incremental patching required before renderer promotion;
- [ ] measurement invalid/not representative — repeat target run.

Rationale: the 5k rebuild is about 2.0 ms and the representative 20k rebuild/forced-eviction rebuild are about 9.0 ms. Same-state cache hits are effectively negligible at 100 ns p95. The measured rebuild behavior does not present a material user-visible latency reason to take on the complexity and correctness risk of incremental PreparedPage mutation before renderer promotion. The existing immutable `PageStateKey`/bounded historical cache model remains the Phase-1 baseline.

## ADR-019 native SVG performance

The retained report was produced by the native Windows Tauri/WebView2 runner on a physical `3840x2160` client area and passed the mechanical gate.

- Runtime: `tauri-webview2`
- Platform: `windows`
- Physical client area: `3840x2160`
- 5k culled update/frame requirement: `pass`
- 20k culled update/frame requirement: `pass`
- Long Task requirement: `pass`
- viewport-bounded DOM requirement: `pass`
- mechanical performance verdict: `performance_gate_pass`

Timing contract: update p95 must be at or below the strict 60 fps renderer-work budget (`1000 / 60 = 16.667 ms`). rAF frame p95 must be at or below `17.500 ms`, preserving the same 60 fps target with only the bounded VSync/timestamp-quantization allowance. The rAF allowance is not additional renderer work budget.

The exact per-case native timing and environment fields remain in the retained renderer report covered by the ADR-019 SHA-256 above. The mechanical verifier result is sufficient for the selection gate and was not manually overridden.

Performance conclusion: **pass**.

## Fidelity evidence identity

- Fidelity manifest: `fidelity/fidelity-20260823-205325-6c5595d62ddb/adr-019-fidelity-evidence.json`
- Fidelity manifest SHA-256: `5b7b065ee8d540e93afa18f5da9de206b084e7458488670f7768582d817aac82`
- SVG SHA-256: `898a269bec5bd9d29eb82fa36ef6acdb061fcd1baa65249b512fcb699fa9ee69`
- Diagnostics SHA-256: `bb997187c3a662503a7296267c8c245355bed193aafd4d7a1a187c008f6b2601`
- Candidate: `render-svg`
- Fidelity eligibility: `true`
- Automated manual-review status before review: `not-reviewed-by-runner`

Expected typed diagnostics present:

- [x] `ConnectorMarkerDeferred`
- [x] `UnsupportedPrimitive`

Unexpected diagnostics: `none reported by the review`.

The deferred connector marker and unsupported polygon remain explicit typed diagnostics. They are not interpreted as silently supported semantics.

## Manual correctness/fidelity review

Review contract: `docs/architecture/adr-019-fidelity-review.md`.

| Check | Result | Notes |
| --- | --- | --- |
| Master layer remains behind page-local layer | correct | — |
| Rounded rectangle and rotations | correct | — |
| Ellipse stroke/fill/alpha | correct | — |
| Horizontal and vertical gradients | correct | — |
| Unicode/XML-sensitive text and page fields | correct | — |
| Supported connector dash styles | correct | — |
| Four rotated page-edge sentinels remain visible/clipped correctly | correct | — |
| Deferred marker remains explicit diagnostic | correct | — |
| Unsupported polygon remains explicit diagnostic | correct | — |
| Review at intrinsic 100% and representative zoom levels | correct | 50%, 100%, 150% and 200% review completed |

Blocking fidelity defects: **none**.

Accepted typed approximations/deferred semantics: `ConnectorMarkerDeferred` and `UnsupportedPrimitive` remain explicit, typed Phase-1 diagnostics and are not promoted to silent fidelity claims.

Manual fidelity conclusion: **acceptable — all required checks correct, zero blocking defects**.

## Renderer decision

Select exactly one:

- [x] **SVG selected as Phase-1 production renderer.**
- [ ] **SVG rejected for Phase 1; evaluate Canvas2D fallback.**
- [ ] **SVG rejected for Phase 1; evaluate WebGL fallback.**
- [ ] **SVG rejected for Phase 1; evaluate Qt/native fallback.**
- [ ] **No decision — evidence must be repeated or architecture work remains open.**

Final rationale: the representative target archive is decision-eligible and clean, the immutable PreparedPage strategy remains acceptable at 5k and 20k, the native Windows/WebView2 SVG path passes the mechanical performance gate, and the manual fidelity review reports every required check correct with zero blocking defects. No evidence-supported reason remains to take on a Canvas2D, WebGL or Qt/native fallback in Phase 1.

This is a Phase-1 production choice, not an irreversible platform commitment. `render-plan`, editor geometry and semantic command/history boundaries remain renderer-independent.

## Promotion consequences

Because SVG is selected:

- the existing evidence-tested SVG adapter is promoted behind a stable production SVG facade;
- production primitive rendering continues through the existing renderer abstraction;
- the renderer-independent `render-plan`, editor geometry and command contracts remain authoritative;
- deferred/approximated semantics remain explicit until separately implemented;
- SVG-specific state must not move into `next-domain` or editor history;
- the evidence-tested internal command/adapter names may remain temporarily for compatibility, but public/desktop production wiring must no longer present SVG as an undecided candidate.

## Phase-1 closure

- [x] combined target evidence is decision-eligible;
- [x] PreparedPage conclusion is recorded;
- [x] native renderer performance conclusion is recorded;
- [x] manual fidelity review is complete with no unresolved blocking defect;
- [x] final renderer decision is recorded above;
- [x] production renderer promotion is implemented by the renderer-promotion PR using the existing measured SVG path;
- [x] public Phase-1 tracking issue #2 is updated to the same source/evidence checkpoint;
- [x] the renderer-promotion pull request references that same evidence checkpoint.

Phase 1 can be marked complete after the production-promotion PR itself passes CI and is merged. The locally retained raw evidence archive remains the reproducible measurement record for source commit `6c5595d62ddb905ed864203230c5a7786b36f860`.
