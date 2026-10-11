# Spike 1: Cross-boundary call overhead (Go → Rust)

**Status:** thresholds pre-registered 2026-10-11 BEFORE any measurement (SPEC §14.7).
**Code:** `work/` (throwaway per SPEC §5 — do not reuse without rewrite).
**Question:** What does one Go→Rust call cost via each candidate mechanism, and which design
tier does Hybrid land in (SPEC §5 Spike 1 decision table)?

## Mechanisms measured

| id | mechanism | what it represents |
|----|-----------|-------------------|
| M0 | `goonly` — plain Go function call | the floor (Go→Go) |
| M1 | `cgo_trivial` — cgo call to empty C function | pure cgo crossing cost (no work) |
| M2 | `cgo_plain` — cgo call to C add function | cgo + a C frame |
| M3 | `asm_direct` — Go asm `CALL` to Rust `extern "C"`, no stack switch | rustgo-style lower bound (unsafe on deep stacks) |
| M4 | `asm_stack` — Go asm trampoline switching to a 1 MiB mmap'd guard-paged stack, then CALL | the rustgo design Hybrid would actually use |

Rust side: `hybrid_add(u64,u64)->u64` and `hybrid_noop()`, `#[no_mangle] extern "C"`,
staticlib, `panic=abort`, release profile. SysV amd64 ABI (RDI, RSI → RAX).

## Pre-registered thresholds (SPEC §5 Spike 1, verbatim decision table)

| Result | Decision |
|--------|----------|
| Under 10ns achievable and stable under panics/stack growth | Keep the trampoline design. |
| 10 to 100ns achievable safely | Accept measured number; compiler batches/inlines across the boundary; update §6 budget. |
| Only cgo-class cost (~30 to 60ns+) is safe | Use cgo as the bridge; hot paths stay on one side; document. |

Operationalized for this spike:
- "Achievable" = M4 (asm_stack, the SAFE variant) median ns/op on the pinned toolchains,
  measured on one tier, reproduced (±25%) on the second tier.
- "Stable" = M4 completes 1e7 calls + 100k mixed-depth calls without crash; stack switch
  correct under Go scheduler (GOMAXPROCS=2, parallel goroutines).
- Sanitize: run the asm_stack path under `go test -race` once (detector may or may not
  understand the stack switch — record honestly).

## Method

- `go test -bench -count=10 -benchmem` (Go 1.26.9, CGO_ENABLED=1), GOMAXPROCS=1, attempt
  `taskset -c 0` pinning (record whether permitted in container).
- Median + min/max spread computed by `work/analyze.py` from raw bench output (benchstat
  not installed on 2-core box; deviation logged in PLAN.md).
- Rust-side reference (Rust→Rust call + Rust→C-ABI call): manual `Instant` loop,
  `std::hint::black_box`, 1e8 iterations. Criterion skipped (2-core/4 GB; deviation logged).
- Both tiers: rerun identical commands on the Kaggle worker (job 002) and an Actions shard;
  report all three; absolute numbers labeled sandbox-measured / worker-measured.

## Raw results (filled after runs — no numbers before this line were seen)

### Sandbox (2 cores / 4 GB, Xeon, go1.26.9, rust1.99.0, sandbox-measured, count=10)

| mechanism | median ns/op | min–max ns/op | allocs/op |
|-----------|-------------:|--------------:|----------:|
| M0 goonly (Go→Go floor) | **0.284** | 0.283–0.288 | 0 |
| Rust→Rust direct (reference, 1e8 iters) | 0.575 | — | 0 |
| Rust→`extern "C"` (reference, 1e8 iters) | 0.574 | — | 0 |
| Rust→indirect C call (reference, 1e8 iters) | 0.600 | — | 0 |
| M1 cgo_trivial (empty C fn) | **46.78** | 46.55–47.13 | 0 |
| M2 cgo_plain (C add) | **52.52** | 52.28–54.39 | 0 |
| M3 asm_direct (goroutine stack) | **2.41** | 2.40–2.43 | 0 |
| M3 asm_direct_noop | 1.95 | 1.94–1.99 | 0 |
| M4 asm_stack (1 MiB guarded mmap stack — SAFE design) | **2.82** | 2.81–2.88 | 0 |
| M4 asm_stack_noop | 2.87 | 2.86–2.90 | 0 |

Ratios: M4 / M0 ≈ 10.0x a Go call; M4 / M1 ≈ 0.060x (16.5x faster than cgo);
M4 / M2 ≈ 0.054x. Spread ≤ 2.5% across 10 runs.

Stability evidence (all PASS on the sandbox):
- `TestAsmCorrectness` incl. 20,000-deep Rust recursion (~640KB+ stack) on the switched stack;
- `TestAsmStabilityUnderScheduler`: 8 parallel goroutines × 10k switched calls, GOMAXPROCS=2;
- `TestAsmGCHammer`: 500k boundary calls + forced GC churn;
- `go test -race` over correctness + scheduler probes: **PASS** (detector saw no races
  through the switch — note: race detector instruments Go code, not the Rust frames;
  it proves the Go-side protocol clean, not Rust-side absence of races).

Linking technique that made M3/M4 possible on Go 1.26 (the 2017 rustgo
`//go:cgo_import_static`+`//go:linkname` trick is rejected by modern Go:
"only allowed in cgo-generated code"):
1. a **cgo package that performs no per-call crossings** — preamble declares the Rust
   fns `extern` + tiny C helpers returning their addresses (`addr_add()` etc.),
   addresses cached in Go vars at package init (one cgo call each, ever);
2. a **pure-Go+asm package** (Go 1.26 forbids cgo and assembly in the same package)
   whose trampolines take the fn pointer as a Go argument and CALL it directly;
3. LDFLAGS from the cgo package link the Rust staticlib; external link mode is implied.
Inspiration: rust2go `asmcall` / petermattis/fastcgo (see research/rust2go.md);
our asm is written for this spike against Go runtime conventions (runtime·systemstack
pattern), not copied.

### Kaggle worker (worker-measured)
(job 002 — to fill)

### Actions runner (runner-measured)
(to fill)

## Analysis & decision
(to fill after all three tiers report)
