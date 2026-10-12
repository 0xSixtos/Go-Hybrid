# STATUS (for the human — 15 lines max)

- Phase: 0 → **COMPLETE. Gate G0 PASSED** (2026-10-12). Phase 1 (SPEC-LANGUAGE.md → MVP-0) is cleared to start.
- Works (verified, both tiers): ALL SIX SPIKES GREEN — compute, call overhead (2.42 ns), Go linking (14/14), memory/handles, scheduling, errors, ownership usability
- Works (verified): Spike 4 worker tier (job 005 v9, all green, 521 s) — at **100,000 tasks**: M2b two-tier at **1.00–1.19× pure-Go** on every cell; M1 starvation theory-exact (capped at 42×+, 93% drained); M3 long cell 19.62 s vs 19.5 s predicted; zero lost tickets
- Works (verified): Spike 6 — Tier-0 ownership REAL for the MVP corpus: 10/10 programs, **zero** lifetime annotations, **zero** `managed`; rustc = CI oracle only
- Three M1 indictments on record: 169× starvation; canary stalls to 5.4 s; the GC-assist wedge (on-stack blocking can freeze allocation itself) — two-tier design is the settled answer
- Thread budgets are environment-specific (sandbox walls at ~717; worker pids.max = 38508, 5262 threads fine) → the bridge derives budgets by probe
- Honest ledger: enter/exit pair cost **~0.9 ms intrinsic on both tiers** (worker refuted the sandbox's syscall-interception theory) — absorbed by design (blocking-only callees, amortized); P3 pin→copy fallback; Rust error-chain loss documented
- research/spike-report.md written (results, chosen design, relaxed targets, cuts); build-vs-extend re-run: **Option B CONFIRMED**
- QUESTIONS.md: 2 human decisions queued (Tier-0 sendability reading; thread-budget config) — defaults proceed per SPEC §9
- Blocked: nothing
- Top risk: R6 (MVP scope) — R1–R5 closed by spikes
- Next 3 actions: (1) SPEC-LANGUAGE.md first draft (A1–A8 resolutions), (2) MVP-0 walking skeleton (example 01 → 09), (3) adversarial checker corpus with pre-registered verdicts
- Last review gate: **G0 PASSED** (next: G1 after language spec + MVP-0)
