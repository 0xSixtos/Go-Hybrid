<div align="center">

# **Hybrid**

**Rust-grade safety. Go-shaped code. One build command.**

*A language that lets teams with an existing Go codebase adopt Rust-grade memory
safety — one file at a time, without a rewrite.*

[![status: experimental](https://img.shields.io/badge/status-experimental-orange)](https://github.com/limitlessprojects/Go-Hybrid/blob/main/PLAN.md)
[![license: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-blue)](LICENSE)
[![CI](https://github.com/limitlessprojects/Go-Hybrid/actions/workflows/ci.yml/badge.svg)](https://github.com/limitlessprojects/Go-Hybrid/actions/workflows/ci.yml)
[![spike gate](https://img.shields.io/badge/phase-0%20%7C%20feasibility%20spikes-8b5cf6)](spikes/)

[Design spec](SPEC.md) · [Research](research/) · [Spike evidence](spikes/) · [Status](STATUS.md) · [Roadmap](PLAN.md)

</div>

---

> **Status — read this first.** Hybrid is **pre-MVP and in the feasibility-spike
> phase**. Nothing is installable yet, and the `.hy` sample below is *target
> syntax*. What exists today and is verified by tests: the compute + linking
> substrate the language will sit on (see [Evidence](#evidence-measured)).
> Live status lives in [STATUS.md](STATUS.md); the plan in [PLAN.md](PLAN.md).

## What it is

Hybrid is a programming language for teams with an existing Go codebase who want
Rust-grade memory safety — incrementally. Write `.hy` files beside your `.go`
files, call a pinned set of Rust crates from Hybrid code, and build a working
binary with a single command. Your Go callers don't change, don't learn a second
build system, and don't need any new toolchain to consume Hybrid packages.

```hy
// checkout.hy — target syntax (not yet implemented)
import "rust/regex"

fn validate_email(raw string) (string, error) {
    re := regex.must_compile(`^[\w.+-]+@[\w-]+\.[\w.]+$`)
    if !re.is_match(raw) {
        return "", errors.new("invalid email")
    }
    return raw, nil
}
```

```go
// main.go — your existing Go code, unchanged
email, err := hybrid.ValidateEmail(raw)
```

```sh
hybrid build && ./myapp    # one command, both toolchains, one binary
```

## Why Hybrid

- **Safe by default.** No data races and no use-after-free in Hybrid's safe mode
  — and safety claims are proven by compile-fail tests, not asserted.
- **Incremental.** Adopt file-by-file inside an existing Go module. No rewrite,
  no big-bang migration project.
- **One build command.** `hybrid build` drives the Go and Rust toolchains for
  you. Consumers of a Hybrid package need *no* Hybrid toolchain at all —
  generated Go source plus a prebuilt Rust artifact is committed ([verified by
  spike 2](spikes/spike2-go-linking/RESULTS.md)).
- **Rust crates, Go ergonomics.** Error handling, concurrency, and generics
  feel native to a Go developer; the crate ecosystem is Rust's.

## How it works (the design bet, and what's already proven)

Hybrid compiles to **Go source** — so it rides the Go compiler, runtime, GC and
scheduler instead of replacing them — and calls Rust through a generated bridge:
a Rust `staticlib` linked into the same binary, reached through hand-emit­ted
assembly trampolines that skip the cgo runtime entirely.

| design question | answer | status |
|---|---|---|
| What can Hybrid emit that Go links? | Go source + `//line` diagnostics; emitted Go assembly for trampolines; binary-only ruled out | ✅ [spike 2: 14/14 green](spikes/spike2-go-linking/RESULTS.md) |
| What does a Go→Rust call cost? | **2.8 ns** measured (safe stack-switched trampoline) vs **46.8 ns** plain cgo — 16.5× faster, 0 allocs | ✅ [spike 1](spikes/spike1-call-overhead/RESULTS.md) |
| Can consumers skip the toolchain? | Yes — committed generated source + prebuilt artifact; Go + a C toolchain suffice (no Rust, no Hybrid) | ✅ [spike 2 E5](spikes/spike2-go-linking/RESULTS.md) |
| Where do builds/tests run? | 2-core home sandbox, a 4-core/30 GB Kaggle worker (42 s round-trip), and a 20-concurrent-runner GitHub Actions fleet — one job spec, one runner | ✅ [spike 0: all green](spikes/spike0-compute/RESULTS.md) |
| Memory model across the boundary | handles/arena design | 🔬 spike 3 (pending) |
| Scheduling with blocking Rust calls | bounded blocking pool vs Go-scheduler fallback | 🔬 spike 4 (pending) |
| Ownership rules without visible lifetimes | Tier-0 checking; borrow-check-by-delegation to rustc under evaluation | 🔬 spike 6 (pending) |

Full prior-art analysis (rustygo, rustgo, rust2go, Borgo, TinyGo, CXX/Crubit,
WIT…) and the build-vs-extend decision live in [research/](research/) — the
recommended path (source-to-source + bridge, scored 59/65) is written up in
[research/build-vs-extend.md](research/build-vs-extend.md).

## Roadmap

Phase 0 — research + feasibility spikes *(in progress; Spikes 0–2 done)*
&nbsp;→&nbsp; language spec + walking skeleton (MVP-0)
&nbsp;→&nbsp; front end + Go back end
&nbsp;→&nbsp; boundary (arena/handles, errors, panic rule)
&nbsp;→&nbsp; concurrency
&nbsp;→&nbsp; stdlib &nbsp;→&nbsp; `hybrid build` DX &nbsp;→&nbsp; hardening &nbsp;→&nbsp; verification.

Checkbox-level detail with dates and exit criteria: [PLAN.md](PLAN.md).

## When *not* to use Hybrid

- You need **arbitrary existing Go code to run unmodified** — that's a different
  project's problem (see rustygo in our research notes).
- You need **every Rust crate** on day one — the target set is crates with
  C-compatible or simple APIs first.
- You need a **1.0 stability contract** today — the language, ABI and CLI will
  change; pin versions and read the changelog.
- Your hot path can't tolerate Go's GC pauses — put that path *in* Rust and call
  it from Hybrid; don't put it in Hybrid-shaped Go.

Honest comparisons, no FUD: staying in Go is excellent for most code; a full
Rust rewrite is right when you can afford it; cgo works today but leaves
unsafe boundaries hand-audited and two build systems hand-run. Hybrid's bet is
*checked* boundaries and *one* command.

## Get it

Research-phase repo; the MVP has landed — **all ten target programs build and
run end-to-end byte-exact** through the Tier-0-checked pipeline (parse →
ownership check → emit Go → `go build`), including the Rust-crate boundary
(05/10: sha3 + serde_json through the live bridge), Go-consumable package
emission (09: a plain `go run` imports a Hybrid-compiled package with no
Hybrid toolchain), and net/http (06). From source:

```bash
git clone https://github.com/limitlessprojects/Go-Hybrid.git && cd Go-Hybrid
# toolchain pins: Go 1.26.9, Rust 1.99.0 (tools/env.sh)
make examples    # all ten target programs, byte-exact vs expected.txt
make test-quick  # 194 compiler tests + checker corpus (20/20) + differential loop (9/9 + 11/11)
make differential # accepted corpus programs must build+run; rejected must be rejected
make bench       # G2 call-overhead bench on the LIVE bridge (fast path 3.37 ns ≤ 5 ns)
hyb build <file.hy> -o <bin>              # one command; enforces the pins (HYB-E9001)
hyb build --package <name> --out-dir <dir> --binding-prefix <path>  # Go-importable package
```

Review gates: G0 passed, G1 ratified, G2 passed + ratified (`gates/G2.md`),
G3 (MVP-exit sweep) **reviewed + passed 2026-10-13 — the MVP is done**
(`gates/G3.md` records the claim-by-claim re-verification: standard
suites — cargo test, go vet, gofmt, cargo clippy — plus the repo's own
acceptance/differential/corpus/bench evidence).


## Learn more

- **Want to use it someday?** Read [SPEC.md](SPEC.md) §1 (mission) and the
  `.hy` samples above; then watch releases.
- **Want to contribute or dig in?** [AGENTS.md](AGENTS.md) (how the repo works),
  [PLAN.md](PLAN.md) (live status + decision log), [research/](research/)
  (prior-art studies), [spikes/](spikes/) (pre-registered hypotheses + raw
  results — including the failures, which are kept on purpose).
- **Skeptical?** Good. Every claim above links to the evidence that backs it,
  and the evidence files state their measurement conditions. Nothing is claimed
  "done" without a test that ran.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) (in progress) and the sub-agent rules in
[AGENTS.md](AGENTS.md). The project follows an evidence-first protocol:
hypotheses are pre-registered before measurement, failures are reported plainly,
and tests are never weakened to pass.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option — the same formula as the Rust ecosystem. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in this
project by you shall be dual-licensed as above, without any additional terms or
conditions.
