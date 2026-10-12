# STATUS (for the human — 15 lines max)

- Phase: 0 — ALL SIX SPIKES DONE (Spike 4 worker tier + spike-report + G0 remain to close the phase)
- Works (verified): Spike 6 — Tier-0 ownership is REAL for the MVP corpus: 10/10 target programs need **zero** lifetime annotations and **zero** `managed` (strictest reading); rustc = CI oracle only
- Works (verified): Spike 4 sandbox — scheduling DECIDED: **two-tier blocking design** (entersyscallblock linkname trampoline, P-release canary-proven, + concurrency-sized bounded pool overflow)
- Works (verified): M2b two-tier tracks M0 within **1.0–1.1×** on 14/15 load cells; zero deadlock/panic under `-race`; threads bounded by design
- Works (verified): M3 pool drains at exactly the predicted concurrency bound (4.01 s vs 3.9 s) with **zero lost tickets** across every uncapped cell
- Measured: naive blocking starves at **169×** (R4 closed); the canary goroutine itself stalls to 5.4 s under it
- CRASH FINDING #2: uncapped syswin dies at ~718 threads — Go Ms never shrink + this environment's address-space wall (M4 hit the same wall independently) → the bridge owns a hard thread budget (50 ms sampler + admission gate)
- Honest caveats on record: naked enter/exit pair ~0.53 ms on this syscall-intercepting sandbox (amortized in-load); M3 poll-latency overhead same-environment tax — job 005 re-measures both on native Linux + runs the 100k matrix
- Recovery: sandbox was reset between sessions; repo re-cloned, toolchains re-pinned, Kaggle creds re-staged; GitHub secret verified intact server-side
- Fixed: committed Makefile had spaces not tabs (every target was unrunnable); `make check` green again
- Blocked: nothing
- Top risk: R6 (MVP scope) — R1–R5 all closed by spikes; worker-tier confirmation of Spike 4 in flight
- Next 3 actions: (1) fetch job 005 (100k tier), (2) research/spike-report.md, (3) gates/G0.md + re-run build-vs-extend → Phase 1
- Last review gate: G0 pending (all six spikes now have data)
