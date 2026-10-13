# STATUS (for the human — 15 lines max)

- Phase: 1 — **IN PROGRESS**. **G1 gate doc WRITTEN (gates/G1.md) — READY FOR HUMAN REVIEW** (language spec + MVP-0 both landed and machine-verified)
- **MVP-0 WALKING SKELETON GREEN (session 5, 2026-10-13): parse → Tier-0 check → emit Go → go build → run on FOUR target programs — 01/02/03/04 all byte-exact vs expected.txt** (SPEC asked for one; four landed)
- Works (verified): example 02's full E6 `?`-exit contract (defers → `cat: open …` on stderr → exit 1) AND byte-verbatim happy path; example 03's structs-first emission + task→go + channels; example 04's select + inclusive `1..=8` + labeled `break loop` + panic
- Works (verified): Tier-0 checker corpus **20/20 verdict agreement**, tripwire not fired (10.0%/15.0% vs ~25%); 147 compiler tests green (`make test-quick`); `hyb build` enforces toolchain pins (HYB-E9001)
- The crashed session's lost work was RECREATED (hy-emit + hyb build + registry appends + example 03 decls) — both recovered emitter bugs have named regression tests
- New tooling: `make examples` (acceptance runner), `tools/check-versions.sh` (version-drift guard, negative-tested), CI `compiler` job (pinned toolchains + tests + corpus + examples on every push)
- Honest scope: 05/10 need the Rust bridge (Phase 2), 07/08 need the enum/match/generics design (next batch), 06 needs method-vs-field type knowledge — all error cleanly today, never wrong output
- Blocked: nothing. G1 ratification pending (emitter lowering decisions listed in gates/G1.md)
- Next 3 actions: (1) human reviews gates/G1.md, (2) enum/option/match design → programs 07/08 (Phase-1 exit), (3) Phase-2 bridge → programs 05/09/10
- Last review gate: G0 PASSED (G1 written, awaits human)
