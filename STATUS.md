# STATUS (for the human — 15 lines max)

- Phase: 0 — research + feasibility spikes (Spikes 0/1/2 **done**, 3–6 next; no compiler code yet, by design)
- Works (verified): toolchains pinned (go1.26.9, rust1.99.0); 3-tier job infra (sandbox / Kaggle worker / 20-runner Actions fleet), one job spec + one runner
- Works (verified): Spike 1 — safe Go→Rust trampoline at **2.82 ns** (worker: 4.36 ns) vs 46.8 ns cgo, 0 allocs, race-clean, GC-hammer-clean
- Works (verified): Spike 2 — Go-source emission with `//line` mapping, emitted asm, single-command build, **zero-install consumption** (14/14)
- Works (verified): 8 prior-art studies → research/claims.md (all SPEC §3 rows verdict'd) + build-vs-extend decision (option B, 59/65)
- Works (verified): ten target programs sketched (examples/); README published; repo CI green
- Blocked: nothing
- Top risk: Spike 3 (arena/GC soundness) is the next hard question; R11 Go-codegen ceiling flagged
- Next 3 actions: (1) Spike 3 arena/handles + interfaces/abi-types.md draft, (2) Spike 5 error representation, (3) Spike 6 ownership usability → then spike-report + G0
- Last review gate: none yet (G0 pending Spikes 3–6)
