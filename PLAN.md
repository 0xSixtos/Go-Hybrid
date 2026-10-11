# Hybrid Language: Development Plan

## Status
- Current phase: 0 (research, claims check, build-vs-extend, spikes)
- State: In Progress
- Last session: 2026-10-11, summary: first session per SPEC.md §15 — environment detected and toolchains pinned, repo scaffolded, research dispatched to parallel agents, Spike 0 (compute tiers) executed, Spikes 1–2 run. Details in Decision Log and Spike Results.

## Environment
- OS/arch: Debian 13 (trixie), x86_64, Linux 5.10
- Go: **go1.26.9** pinned at /home/z/tools/go (do not float)
- Rust: **1.99.0** via rustup minimal profile, pinned by tools/env.sh (rust-toolchain.toml lands with the first compiler crate)
- C compiler/linker: gcc 14.2.0 (Debian), GNU ld
- Hardware (measured 2026-10-11): 2 cores (Xeon, 1 thread/core), **4 GB RAM** (SPEC assumed up to 10 GB — CORRECTED: free -m shows 4159 MB total, no swap), 9.9 GB disk (8.4 GB free after toolchain installs)
- Kaggle: CLI 2.2.4, KGAT token at ~/.kaggle/access_token (verified: `kaggle kernels list --mine` OK)
- GitHub: repo public (push confirmed), Actions available; gh CLI not installed — REST API via curl used instead

## Design Decisions in Force
- Build-vs-extend choice: pending Phase 0 (see research/build-vs-extend.md when written)
- Go-side output: pending Spike 2
- Cross-boundary memory model: pending Spike 3
- Scheduling model: pending Spike 4
- Ownership model: pending Spike 6
- License: dual MIT/Apache-2.0 (SPEC §12 default; user did not override) — 2026-10-11
- Justfile vs Makefile: **Makefile** chosen — `just` not preinstalled and a 2-core cargo install is not free; make is ubiquitous. Revisit if `just` pays later. — 2026-10-11

## Claims Verification
Tracked in research/claims.md (agent-filled; spot-check before building on any claim).

## Spike Results
| Spike | Threshold | Result | Decision |
|-------|-----------|--------|----------|
| 0 Kaggle | end-to-end job works | see spikes/spike0-compute/RESULTS.md | |
| 0 Actions | 4→20 shards merge | see spikes/spike0-compute/RESULTS.md | |
| 1 call overhead | <10ns / 10–100ns / cgo-class | see spikes/spike1-call-overhead/RESULTS.md | |
| 2 Go linking | source > asm > objects | see spikes/spike2-go-linking/RESULTS.md | |

## Task List
- [x] Session 1 scaffold (this repo structure) | done = files exist, pushed | lead | done
- [x] Environment detect + pin toolchains | done = PLAN.md records versions | lead | done
- [ ] research/claims.md filled from prior-art study | done = every §3 table row CONFIRMED/CORRECTED/UNVERIFIABLE | agents | in progress
- [ ] research/build-vs-extend.md | done = 4 options scored, one recommended with rationale | lead | in progress
- [ ] Spike 0 compute smoke (Kaggle + Actions) | done = RESULTS.md with measured latencies | lead | in progress
- [ ] Spike 1 call overhead | done = benchmarks with medians+spread vs thresholds | lead | in progress
- [ ] Spike 2 Go linking routes | done = each route tested, one chosen | lead | in progress
- [ ] Ten target programs sketched in examples/ | done = each has .hy intent + expected output | lead | pending
- [ ] README.md (top-repo quality, user-requested) | done = patterns doc + README reviewed | lead | in progress

## Blockers
- (none currently; RAM is 4 GB not 10 GB — mitigation: stricter -j2, timeout wrappers, check free -m before heavy jobs)

## Risk Register
| ID | Risk | Likelihood | Impact | Mitigation / check |
|----|------|-----------|--------|--------------------|
| R1 | Safe cross-boundary call cost far above target | M | H | Spike 1 |
| R2 | Go output linking is brittle across Go versions | M | H | Spike 2, pin 1.26.9 |
| R3 | Arena/GC interaction is unsound | M | H | Spike 3, negative tests |
| R4 | Blocking Rust calls starve the scheduler | M | M | Spike 4 |
| R5 | "No visible lifetimes" ownership model unrealistic | M | H | Spike 6 |
| R6 | MVP scope too large for available effort | H | H | MVP-0, cut order |
| R7 | Sandbox resource exhaustion (4 GB RAM, 9.9 GB disk) | H | H | Resource Governor + env.sh |
| R8 | Kaggle worker unreliable or restricted | M | M | Spike 0, fallback |
| R9 | Actions minutes exhausted / disabled for heavy usage | L | M | public repo = free minutes; ToS care |
| R10 | Token leakage into public repo | L | H | .gitignore guards; never echo tokens; audit pushes |

## Review Gates
- G0: pending | G1: [ ] | G2: [ ] | G3: [ ]

## Test Results
(none yet — no compiler code exists; spike results live in spikes/)

## Decision Log
- 2026-10-11 | Pin go1.26.9 (not 1.27.2) | alternatives: latest 1.27.2 | rationale: patch-mature line, one behind HEAD; spec favors stability for ABI-sensitive work; differential corpus matches mature stdlib | reverse: change PINNED_GO in tools/env.sh + record
- 2026-10-11 | Makefile over justfile | alternatives: cargo install just | rationale: zero install cost on 2-core box | reverse: add justfile later if it pays
- 2026-10-11 | License = dual MIT/Apache-2.0 | alternatives: MIT only | rationale: SPEC §12 default, Rust-ecosystem convention | reverse: user decision logged here
- 2026-10-11 | No gh CLI; use curl + REST with token from .git/config context | alternatives: install gh | rationale: REST covers workflow_dispatch + run polling; less disk | reverse: apt/gh install if ergonomic need arises

## Cut Log
(empty)

## Dependencies Added
- kaggle CLI 2.2.4 (pypi, in /home/z/.venv) — reason: worker tier protocol; license: Apache-2.0 (kaggle-api)
- (rustup + toolchains are environment, not project deps; no cargo crates added yet)
