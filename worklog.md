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

---
Task ID: 9 (session 5 — batch D + CI fix + docs)
Agent: lead (Super Z)
Task: programs 07/08 (Phase-1 exit), CI fix, README/docs refresh

Work Log:
- Batch D: option[T] prelude (val+ok struct, some() ctor, none=zero; return none lowers vs declared result); option-match → hyMatchN temp + .ok/.val if-else (guard = nested if); set[T] prelude (insert returns is-new); .pop() → hy_pop(&x); TypeAlias decl (type node_id = int — was a zero-field struct approximation); assoc fns mangle lru_cache.new → lru_cache_new; generic fns emit [k comparable, v any]; single unnamed results bare
- O8 map-slot loan desugar: x := &m[k] → copy-read + store-back at declaring block's end (Go cannot &m[k]; O9 honesty made operational; block-scoped loan_frames flushed via emit_block_body, ALL block sites routed through it)
- Chan binding hint: single-var iterate binds the VALUE — slices/maps emit '_, v', channels keep 'v' (hints from chan params + make)
- Parser: peek_generic_args accepts single type-shaped args (box[t]{v: 1}); ambiguity documented (ident[index](/{ favors generics; index-call shapes absent from surface)
- Example 07: touch() implemented per its own intent comment; Example 08: stack := []node_id{from} (bare-ident slice literal needs full local inference — deferred + logged)
- 157 tests (48 emit + 10 batch-D tests); corpus 20/20; examples 6/6 PASS
- CI failure diagnosed via API logs: runner rustup shim (no default toolchain) shadowed the fresh install → check-versions now tolerates empty versions + CI step prepends the installed toolchain PATH
- README 'Get it' rewritten (MVP-0 landed: real quickstart); PLAN/STATUS/worklog refreshed

Stage Summary:
- Phase-1 exit criteria MET (programs 1, 2, 7, 8 compile + match expected)
- Commits this session: 9b2fe31 → 97285d3 → 8639e0b → 1f8ad9e → 2bebc0c → 4000d4b → cfee21f (CI fix) → final docs commit
- Remaining plan work: Phase 2 (Rust bridge: programs 05/09/10 + call-overhead bench) — new phase, awaits G1 human review per the gate discipline
