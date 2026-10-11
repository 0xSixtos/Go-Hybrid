# STATUS (for the human — 15 lines max)

- Phase: 0 — research + feasibility spikes (Spikes 0/1/2/3 **done**, 4–6 next; no compiler code yet, by design)
- Works (verified): Kaggle key stored as GitHub Actions secret + kaggle.yml — Actions can now drive ALL 3 tiers (104 s end-to-end run green)
- Works (verified): Spike 3 — owning-side handle tables: create/release 49.9 ns (0 allocs), get 0.84 ns, Rust-side resolve 14.1 ns, 100k-object dual-release storm green under `-race`
- Works (verified): crossing REDESIGNED after a real crash — fast path runs shallow Rust on the caller's own goroutine stack behind a stackguard check (**2.42 ns**), deep path on bitmap-pooled 1 MiB guarded stacks (10k-deep recursion green)
- Finding: `runtime.Pinner` costs ~55 ns per pin/unpin pair (> cgo) — pre-registered fallback fired: copy small values at the boundary, pin only large buffers
- Finding (hazards demonstrated): unpinned heap pointer read back as churn poison (UAF); captured stack pointer overwritten — raw pointers stay forbidden in safe mode
- Works (verified): 2 research notes landed — runtime-hazards.md (all 9 SPEC §12 checklist items decided) + ownership-models.md (Spike 6 design: Polonius-style engine, rustc as CI oracle)
- Security: ephemeral Kaggle JWTs found in 2 old job summaries → redacted; runner now masks secret-looking env values; CI scan catches JWT shapes
- Blocked: nothing
- Top risk: R4 (crossing holds the P during foreign calls — Spike 4's question); R5 (Tier-0 ownership realism — Spike 6)
- Next 3 actions: (1) Spike 4 scheduling/blocking load, (2) Spike 5 error representation, (3) Spike 6 pseudo-Hybrid programs → spike-report + G0
- Last review gate: none yet (G0 pending Spikes 4–6)
