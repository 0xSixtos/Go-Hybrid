# STATUS (for the human — 15 lines max)

- Phase: 1 — **IN PROGRESS**. Gate G0 PASSED + **both human rulings received (2026-10-12)**: Tier-0 sendability = **strict (c)+(b)**, provisional, six binding additions (adversarial corpus = the spec, diagnostic fix-it errors, auditable `managed` + `--report-managed`, >~25% tripwire, claim scoped to checked subset); thread budget = configured ceiling + lower-only probes (default `min(512, 0.5×pids.max)`, source logged at startup)
- Works (verified, both tiers): ALL SIX SPIKES GREEN — compute, call overhead (2.42 ns), Go linking (14/14), memory/handles, scheduling, errors, ownership usability
- Works (verified): Spike 4 worker tier (job 005 v9, all green, 521 s) — at **100,000 tasks**: M2b two-tier at **1.00–1.19× pure-Go** on every cell; M1 starvation theory-exact (capped at 42×+, 93% drained); M3 long cell 19.62 s vs 19.5 s predicted; zero lost tickets
- Works (verified): Spike 6 — Tier-0 ownership REAL for the MVP corpus: 10/10 programs, **zero** lifetime annotations, **zero** `managed`; rustc = CI oracle only
- Three M1 indictments on record: 169× starvation; canary stalls to 5.4 s; the GC-assist wedge (on-stack blocking can freeze allocation itself) — two-tier design is the settled answer
- Thread budgets: **DECIDED** — configured ceiling (`max_threads`, default `min(512, 0.5 × pids.max)` when readable); probes/rlimit may only LOWER the effective budget (tested invariant); source logged at startup; sandbox ~717 wall / worker pids.max 38508 both honored
- Honest ledger: enter/exit pair cost **~0.9 ms intrinsic on both tiers** (worker refuted the sandbox's syscall-interception theory) — absorbed by design (blocking-only callees, amortized); P3 pin→copy fallback; Rust error-chain loss documented
- research/spike-report.md written (results, chosen design, relaxed targets, cuts); build-vs-extend re-run: **Option B CONFIRMED**
- QUESTIONS.md: **empty** — both questions resolved by the human; escalation bar tightened (expensive-to-reverse only)
- Competitive strategy adopted (human-supplied, research/positioning.md): match GALA/Lisette on type safety (table stakes), beat on Rust interop (differentiator), never break compile speed/simplicity; pitch = "Keep Go exactly as it is. Add Rust where you need it."
- Phase 1 core artifacts LANDED (2026-10-12): **SPEC-LANGUAGE.md** (2,056 lines, 13/13 §8 areas, ~137 rules, A1–A8 resolved, `HYB-Exxxx` registry, D1–D17 open ledger); **adversarial checker corpus** (tests/checker-corpus/, 20 programs, verdicts pre-registered BEFORE the checker: ACCEPT 7 / ACCEPT-MANAGED 2 / REJECT 11; managed tripwire baseline 10.0% verdict-level, 15.0% as-written — both under 25%, G0 stays closed); **competitive DX study** (873 lines, GALA/Soppo/Lisette, 17-item evidence-backed copy table, post-MVP DX queue ordered)
- Blocked: nothing
- Top risk: R6 (MVP scope) — R1–R5 closed by spikes
- Next 3 actions: (1) diagnostic registry v1 scaffold (JSON envelope + seed codes + `hybrid explain` stub — SPEC §9.6 now normative), (2) checker harness consuming the corpus's `EXPECTED-VERDICT` lines + `--report-managed` counter, (3) Tier-0 checker MVP → corpus 20/20
- Last review gate: **G0 PASSED** (next: G1 after language spec + MVP-0)
