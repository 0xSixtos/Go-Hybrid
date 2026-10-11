# Hybrid Language: Development Plan

## Status
- Current phase: 0 (research, claims check, build-vs-extend, spikes) — Spikes 0–3 complete; 4–6 pending
- State: In Progress
- Last session: 2026-10-12, summary: session 2. KAGGLE_API_TOKEN stored as GitHub Actions secret + kaggle.yml workflow (Actions→Kaggle end-to-end verified, 104 s); ephemeral Kaggle JWT leak found in old job summaries, redacted + runner env-report now masks secret-looking values + CI hygiene scan extended to JWT patterns. Research: runtime-hazards.md (§12 checklist — test-or-decision for all 9 items) + ownership-models.md (Spike 6 study: Polonius-style engine + Mojo conventions + WIT handles; rustc as CI oracle only). Spike 3 PASS with one pre-registered fallback: owning-side handle tables adopted (P1 49.9 ns/0 alloc, P2 0.84 ns, P4 14.1 ns, P5/P6/P9 green, gen-tagged ids); P3 pinned-borrow FAILED (+55 ns — Pinner costs more than cgo) → copy small values, pin large buffers; hazard probes P7/P8 demonstrated concretely. CRASH FINDING: Spike 1's shared mmap stack is concurrency-unsafe (cleanup goroutines) → crossing redesigned: guard-checked fast path on caller's own goroutine stack (2.42 ns compiler projection, offsets pinned+canary) + bitmap-pooled 1 MiB deep stacks. interfaces/abi-types.md draft written. Worker tier (job 003) submitted after this push.

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
- **Cross-boundary memory model: HANDLES (owning-side tables, gen-tagged) + copy-small/pin-large borrows + opt-in scoped arenas** (Spike 3, 2026-10-12; P3 fallback pre-registered and fired)
- **Crossing (Spike 3 redesign):** fast path = guard-checked direct call on caller's goroutine stack (2.42 ns, go1.26.9 runtime.g offsets pinned + canary); deep path = bitmap-pooled 1 MiB guarded mmap stacks. Spike 1's shared single mmap stack RETIRED (concurrency-unsafe — real crash found by Spike 3 cleanup test). 2026-10-12
- **Scheduling model: PENDING Spike 4** (bounded blocking pool leading candidate; note: current crossing holds the P during foreign calls — Spike 4 question)
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
| 3 memory/arena | per pre-registered table | **PASS with fallback**: P1/P2/P4/P5/P6/P9 green; P3 FAILED (+55 ns pin cost) → copy-small/pin-large; hazards P7/P8 demonstrated | handles + gen-tags + pinned-large-borrow; abi-types.md draft |
| 4 scheduling | pending | — | — |
| 5 errors | per pre-registered table | **PASS all**: E1 2.58 ns/0 allocs (two-word HyResult), E2 0 allocs, E3 1 alloc, E4 identity + documented chain loss, E5 abort contract verified | adopt HyResult + sentinel table + wrap-on-demand |
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
- [x] Spike 3 memory across the boundary (arena/handles) | done = working code + tests + interfaces/abi-types.md draft | lead | done (sandbox; worker tier = job 003)
- [ ] Spike 4 scheduling with blocking foreign calls | done = 1k/10k sandbox + 100k worker | lead | pending
- [x] Spike 5 error representation | done = allocation counts measured | lead | done (sandbox; worker tier = job 004)
- [ ] Spike 6 ownership usability | done = 10 programs pseudo-Hybrid'd + annotation count | lead | pending (research done: research/ownership-models.md)
- [ ] research/spike-report.md + gates/G0.md | done = all spikes closed | lead | pending
- [ ] SPEC-LANGUAGE.md first draft | done = grammar skeleton + type/error/concurrency sections | lead | pending (after G0)

## Blockers
- (none)

## Risk Register
| ID | Risk | Likelihood | Impact | Mitigation / check |
|----|------|-----------|--------|--------------------|
| R1 | Safe cross-boundary call cost far above target | **closed** (2.8–4.4 ns) | — | Spike 1 done |
| R2 | Go output linking brittle across Go versions | M | M | Go-source emission (spike 2); pin 1.26.9; CI matrix later |
| R3 | Arena/GC interaction unsound | **closed** (spike 3: storm + sentinel + gen-tags green) | — |
| R4 | Blocking Rust calls starve scheduler | M | M | Spike 4 (crossing holds P during foreign calls — measured next) |
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
- Spike 3: 11/11 safe probes PASS under -race; P7/P8 hazards demonstrated; medians: fast crossing 2.42 ns, deep switch 3.98 ns, P1 49.9 ns, P2 0.84 ns, P4 14.1 ns, pin/unpin +55.3 ns
- Spike 5: 4/4 probes PASS under -race (incl. panic contract via child SIGABRT); E1 wide crossing 2.58 ns/0 allocs, E2 sentinel 0 allocs, E3 wrap 1 alloc, E6 rust refs 2.05/1.76 ns/iter
- Fleet smoke: 4/4 and 20/20 shards green at pinned SHA cd21089 (runs 38163370515, 38163372872)
- Actions→Kaggle secret path: kaggle.yml run 38167716652 green (104 s end-to-end)
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
- 2026-10-12 | KAGGLE_API_TOKEN as GitHub Actions secret; kaggle.yml dispatch workflow | user request; one code path (tools/job.py) for all tiers | reverse: rotate token + delete secret
- 2026-10-12 | Runner env report redacts secret-looking values; CI scan extended to JWT shapes | R10: ephemeral Kaggle JWTs leaked into 2 committed summaries (redacted same day, tokens session-scoped/expired) | reverse: n/a
- 2026-10-12 | Spike 1 shared mmap stack RETIRED for concurrency; fast path on caller's goroutine stack + pooled deep stacks | real crash (cleanup goroutines) + measured 2.42 ns fast path | reverse: per-M stacks via TLS g if pooled deep path ever shows contention
- 2026-10-12 | Memory model = owning-side handle tables (gen-tagged ids) + copy-small/pin-large borrows | P3 pre-registered fallback fired (Pinner +55 ns > cgo); P1/P2/P4/P5/P6/P9 pass | reverse: revisit if Pinner gets cheaper in a future Go
- 2026-10-12 | runtime.g offsets (stackguard0) pinned + DWARF extractor + canary test | fast-path guard check needs one offset; version bump → re-run extractor | reverse: cgo fallback if a Go release breaks asm assumptions
- 2026-10-12 | Error ABI = two-word HyResult (RAX:RDX) + sentinel table + wrap-on-demand; panic = abort at Rust edge | Spike 5 all-green; +0.16 ns for (T, error) | reverse: catch_unwind-at-edge only if abort proves unacceptable at G1
- 2026-10-12 | Job commands never pipe into head/tail on live producers (SIGPIPE silently kills the suite; also masks exit codes) | job 003's first run died silently to a gofmt gate + masked pipeline | reverse: n/a — runner already truncates output itself

## Cut Log
(empty)

## Dependencies Added
- kaggle CLI 2.2.4 (pypi, /home/z/.venv) — worker tier protocol — Apache-2.0 (kaggle-api)
- (no cargo crates in the project yet; spike crates rustrt/rustbench are dependency-free stdlib-only)
