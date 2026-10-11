# Hybrid Language: Development Plan

## Status
- Current phase: 0 (research, claims check, build-vs-extend, spikes) — Spikes 0, 1, 2 complete; 3–6 pending
- State: In Progress
- Last session: 2026-10-11, summary: first full session per SPEC.md §15. Toolchains pinned (go1.26.9, rust 1.99.0); 8 prior-art research notes + claims.md + build-vs-extend.md (recommendation: B, source-to-source + bridge, 59/65); 3-tier job infrastructure (one job.json, one runner, sandbox/Kaggle/Actions) verified end-to-end; Spike 0 PASS both parts (Kaggle: 4 cores/30 GB, 42 s round-trip; Actions: 20/20 concurrent shards); Spike 1 PASS row 1 (<10 ns: 2.82 ns sandbox / 4.36 ns worker, safe trampoline, race-clean); Spike 2 PASS 14/14 (Go-source emission, //line mapping, zero-install, binary-only dead); ten target programs written; README published. Spikes 3–6 remain for G0.

## Environment
- OS/arch: Debian 13 (trixie), x86_64, Linux 5.10
- Go: **go1.26.9** pinned at /home/z/tools/go (do not float)
- Rust: **1.99.0** via rustup minimal profile; pinned by tools/env.sh (rust-toolchain.toml lands with the first compiler crate)
- C compiler/linker: gcc 14.2.0 (Debian), GNU ld
- Hardware (measured 2026-10-11): 2 cores (Xeon, 1 thread/core), **4 GB RAM** (SPEC assumed up to 10 GB — CORRECTED), 9.9 GB disk
- Kaggle worker (measured, Spike 0): **4 cores** (2c×2t Xeon 2.2 GHz — CORRECTED from "reportedly 16"), 30 GB cgroup RAM, internet OK, ~11 s queue, ~50 s round-trip for small jobs, zone europe-west4-c
- GitHub Actions (measured, Spike 0): 20/20 concurrent standard runners, **4 vCPU/16 GB** each (public repo — matches spec's public-repo guess), ~10–17 s queue, artifacts ~3 KB
- Kaggle: CLI 2.2.4, KGAT token at ~/.kaggle/access_token (verified)
- GitHub: repo public (push confirmed); no gh CLI — REST via curl (token lives only in .git/config)

## Design Decisions in Force
- **Build-vs-extend: OPTION B (provisional)** — Hybrid → Go source; Rust via generated bridge (address-broker + trampolines + staticlib). Scored 59/65 vs A=34, C=37, D=27. Revisit at G0 after Spikes 3–6. 2026-10-11, research/build-vs-extend.md
- **Go-side output: Go source** (Spike 2: 14/14; binary-only ruled out; asm for trampolines with per-arch build-tag gating). 2026-10-11
- **Cross-boundary memory model: PENDING Spike 3** (handles/arena leading candidate per WIT research + rust2go lessons)
- **Scheduling model: PENDING Spike 4** (bounded blocking pool leading candidate)
- **Ownership model: PENDING Spike 6** (Tier-0 no-visible-lifetimes goal; borrow-check-by-delegation to rustc under evaluation)
- **Crossing design (from Spike 1): asm trampoline + mmap'd guarded stack + address-broker cgo package** — replaces the 2017 rustgo cgo_import_static trick which modern Go rejects. Budget: Hybrid→Rust ≤ 5 ns. 2026-10-11
- License: dual MIT/Apache-2.0 (SPEC §12 default). 2026-10-11
- Makefile over justfile (2-core install cost). 2026-10-11
- Implementation language of the compiler: **Rust** (SPEC §4 default; Kani path). Small crates, cargo check loop in-sandbox, full builds on tiers. 2026-10-11
- Job protocol: one job.json + one runner (tools/runner/runner.py) across all three tiers; Kaggle kernel never changes (tools/job.py injects). 2026-10-11

## Claims Verification
All SPEC §3 rows verified — see research/claims.md. Headlines: rustygo 127/141 tests + GOMAXPROCS corruption CONFIRMED (0/20 at =4); rustgo repo DELETED, cost CORRECTED to 4.6–5.1 ns (not 40); rust2go async CONFIRMED, ByteDance CONFIRMED-with-nuance; Borgo dead (no license, bus factor 1); TinyGo = black-box only; CXX Option limit CONFIRMED, Crubit CORRECTED (bridges optional); WIT = IDL yes / literal ABI no.

## Spike Results
| Spike | Threshold | Result | Decision |
|-------|-----------|--------|----------|
| 0 Kaggle | end-to-end works | **PASS** (42 s round-trip; 4c/30 GB; internet OK) | adopt protocol, corrected facts |
| 0 Actions | 4→20 shards merge | **PASS** (20/20 concurrent, clean merge) | adopt as written |
| 1 call overhead | <10ns / 10–100 / cgo-class | **PASS row 1**: 2.82 ns sandbox, 4.36 ns worker (safe M4), 0 allocs, race-clean | keep trampoline design; §6 budget → ≤5 ns |
| 2 Go linking | source > asm > objects | **PASS 14/14** | Go-source emission primary; zero-install via committed source+artifact |
| 3 memory/arena | pending | — | — |
| 4 scheduling | pending | — | — |
| 5 errors | pending | — | — |
| 6 ownership usability | pending | — | — |

## Task List
- [x] Session 1 scaffold | done = files exist, pushed | lead | done
- [x] Environment detect + pin toolchains | done = this file records versions | lead | done
- [x] research/ claims verification | done = research/claims.md, every §3 row verdict'd | agents | done
- [x] research/build-vs-extend.md | done = 4 options scored + recommendation | lead | done
- [x] Spike 0 compute smoke (Kaggle + Actions) | done = RESULTS.md with measured latencies | lead | done
- [x] Spike 1 call overhead | done = medians+spread vs thresholds, 2 tiers | lead | done
- [x] Spike 2 Go linking routes | done = each route tested (14 checks) | lead | done
- [x] Ten target programs sketched in examples/ | done = each has .hy intent + expected output | lead | done
- [x] README.md (top-repo quality, user-requested) | done = patterns from research/readme-study.md applied | lead | done
- [ ] Spike 3 memory across the boundary (arena/handles) | done = working code + tests + interfaces/abi-types.md draft | lead | next session
- [ ] Spike 4 scheduling with blocking foreign calls | done = 1k/10k sandbox + 100k worker | lead | pending
- [ ] Spike 5 error representation | done = allocation counts measured | lead | pending
- [ ] Spike 6 ownership usability | done = 10 programs pseudo-Hybrid'd + annotation count | lead | pending
- [ ] research/spike-report.md + gates/G0.md | done = all spikes closed | lead | pending
- [ ] SPEC-LANGUAGE.md first draft | done = grammar skeleton + type/error/concurrency sections | lead | pending (after G0)

## Blockers
- (none)

## Risk Register
| ID | Risk | Likelihood | Impact | Mitigation / check |
|----|------|-----------|--------|--------------------|
| R1 | Safe cross-boundary call cost far above target | **closed** (2.8–4.4 ns) | — | Spike 1 done |
| R2 | Go output linking brittle across Go versions | M | M | Go-source emission (spike 2); pin 1.26.9; CI matrix later |
| R3 | Arena/GC interaction unsound | M | H | Spike 3 next; negative tests |
| R4 | Blocking Rust calls starve scheduler | M | M | Spike 4 |
| R5 | "No visible lifetimes" unrealistic | M | H | Spike 6; rustc-delegation fallback |
| R6 | MVP scope too large | M | H | MVP-0 first; cut order §7 |
| R7 | Sandbox resource exhaustion (4 GB RAM) | H | H | -j2 + timeouts + tiers (working all session) |
| R8 | Kaggle worker unreliable | **closed** (verified) | — | Spike 0 done |
| R9 | Actions minutes/ToS | L | M | public repo = free standard minutes; log usage; no abuse |
| R10 | Token leakage into public repo | L | H | .gitignore guards + CI secret scan + never-echo discipline (clean so far) |
| R11 | Go codegen ceiling (GC pauses) misses perf budget | M | M | hot paths via ≤5ns Rust calls; macro-benchmarks at Phase 4 |

## Review Gates
- G0: pending (needs Spikes 3–6) | G1: [ ] | G2: [ ] | G3: [ ]

## Test Results
- Spike 1 probes (correctness / 20k-deep recursion / scheduler-migration / GC-hammer / race): PASS on sandbox AND Kaggle worker
- Spike 2 emission suite: 14/14 PASS (sandbox)
- Fleet smoke: 4/4 and 20/20 shards green at pinned SHA cd21089 (runs 38163370515, 38163372872)
- Differential corpus: not started (Phase 1)

## Decision Log
- 2026-10-11 | Pin go1.26.9 not 1.27.2 | patch-mature line; stability over freshness | reverse: change PINNED_GO + record
- 2026-10-11 | Makefile over justfile | zero install cost | reverse: add justfile if it pays
- 2026-10-11 | License dual MIT/Apache-2.0 | SPEC §12 default | reverse: user decision
- 2026-10-11 | No gh CLI; curl+REST | REST covers dispatch/polling | reverse: install gh if ergonomics demand
- 2026-10-11 | Build-vs-extend = B (provisional) | evidence above | reverse: at G0 with spikes 3–6 data; spike 1/2 artifacts are route-agnostic
- 2026-10-11 | Crossing = address-broker + trampoline (not cgo, not 2017-rustgo-trick) | cgo_import_static rejected by go1.26; measured 2.82 ns safe | reverse: if a Go release breaks asm assumptions, cgo fallback is the §5 row-3 path
- 2026-10-11 | Benchmark spread via count=10 + python stats (benchstat not installed; criterion skipped) | 2-core box | reverse: install benchstat on a worker and re-analyze
- 2026-10-11 | Commit spike work/ source (gitignore artifacts only) | workers must run spikes remotely | reverse: n/a

## Cut Log
(empty)

## Dependencies Added
- kaggle CLI 2.2.4 (pypi, /home/z/.venv) — worker tier protocol — Apache-2.0 (kaggle-api)
- (no cargo crates in the project yet; spike crates rustrt/rustbench are dependency-free stdlib-only)
