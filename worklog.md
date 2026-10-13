# Go-Hybrid — multi-agent worklog (append-only)

---
Task ID: 6 (session 5 — recovery + MVP-0)
Agent: lead (Super Z)
Task: recover crashed session's lost work, finish MVP-0 walking skeleton, continue plan

Work Log:
- Recovered state: cloned repo (PAT verified), read PLAN/STATUS/AGENTS + AST contract + corpus harness baseline (109 tests, 20/20 corpus, tripwire 10/15%)
- Re-pinned toolchains in fresh sandbox: Go 1.26.9 (/home/z/tools/go), Rust 1.99.0 (rustup minimal); background installs die silently in this sandbox — foreground curl+tar+rustup works
- Recreated the lost hy-emit crate from scratch (lib.rs context/file/fn assembly, stmt.rs, expr.rs, 35 tests): two-pass emission fixes recovered bug #1 (imports always before type decls), make() first-arg-as-type fixes bug #2
- Implemented SPEC E3 ?-desugar (unique hyErrN temps, named-result reuse) + E6 main exit contract (closure wrapper, defers-then-stderr-then-exit-1) + M4 normalization + binding hints (Foreign/Bytes) + interp→Sprintf (%% escaping) + task→go + move-marker drop
- Wired hyb build driver: version-pin enforcement (HYB-E9001, option_env pins + HYB_ALLOW_TOOLCHAIN_DRIFT hatch), parse→check→emit→go build
- Registry append-only +2 codes: HYB-E0002 parse-error, HYB-E9001 toolchain-mismatch (19 total)
- Completed example 03's lost struct decls (job/result) from the spike-6 twin
- tools/run_examples.sh + make examples + make test-quick (cargo test + corpus harness); Makefile recipe tabs fixed (spaces→tabs, prior session's logged failure mode)
- E2E verified: 01-hello exact match; 02-cli-cat happy path (bytes verbatim, exit 0) + error path (stderr "cat: open …: no such file or directory", exit 1); 03-worker-pool exact match [2 2 4 6 8 10 12 18]
- COMMITTED 9b2fe31 and pushed (MVP-0 walking skeleton milestone)

Stage Summary:
- Walking skeleton DONE: parse→check→emit→go build→run green on 01/02/03
- 144 tests green; corpus 20/20; tripwire not fired
- Both recovered emitter bugs have named regression tests (bug1 emission order, bug2 make type)
- Pending: version-drift script (PLAN), PLAN/STATUS refresh, batch B assessment (examples 04–09 need AST/parser extensions: inclusive ranges, break labels, select, generics/match, type aliases; 05/10 need the Rust bridge)

---
Task ID: 7-8 (session 5 — batches B/C)
Agent: lead (Super Z)
Task: version-drift script, batch B (example 04 surface), batch C (CI + docs + G1)

Work Log:
- tools/check-versions.sh: env.sh as single source of truth; cross-refs README/hyb defaults/PLAN; probes installed toolchains; --no-toolchain for CI; NEGATIVE-TESTED (injected drift caught); wired into make check
- Batch B (example 04): AST contract extension (lead-owned, logged): ForKind::Range.inclusive + Break/Continue.label; parser captures '..=' and 'break loop' (loop-keyword only — lexer skips newlines so bare-ident labels would misparse the next statement); emitter: hyLoop: labels + break hyLoop (loop is a Go keyword), select → Go case clauses 1:1, panic form
- Example 04 END-TO-END: total=204 == expected.txt (inclusive range correct: 1+4+9+...+64)
- run_examples.sh generalized (4 examples ALL PASS); emit tests 38 (select/inclusive/labeled-break/panic); 147 total; corpus 20/20 intact
- Batch C: CI compiler job (installs pinned toolchains from env.sh-discipline URLs, runs check-versions + cargo test + corpus + examples); gates/G1.md written (READY FOR HUMAN REVIEW); PLAN.md task list all-done through MVP-0 + 9 new Decision Log entries; STATUS.md rewritten; commits after EVERY batch per the no-data-loss directive

Stage Summary:
- Commits: 9b2fe31 (MVP-0 milestone) → 97285d3 (version script) → 8639e0b (Makefile tabs) → 1f8ad9e (example 04 + batch B) → batch C commit pending
- PLAN.md task queue now: enum/option/match + generics (07/08) then Phase-2 bridge (05/09/10)
- Sandbox lesson (repeat): background installs die silently; foreground curl+tar+rustup works
