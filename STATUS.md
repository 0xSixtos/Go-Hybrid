# STATUS (for the human — 15 lines max)

- Phase: 0 — research + feasibility spikes (no compiler code yet, by design)
- Works (verified): env pinned (go1.26.9, rust 1.99.0); Kaggle API auth; GitHub push; repo scaffold
- Works (verified): spike results — see spikes/spike0-compute/RESULTS.md, spikes/spike1-call-overhead/RESULTS.md, spikes/spike2-go-linking/RESULTS.md
- Blocked: nothing
- Top risk: 4 GB RAM sandbox (spec assumed 10 GB) — mitigated via -j2 + timeouts + remote tiers
- Next 3 actions: (1) finish research/claims.md synthesis, (2) research/build-vs-extend.md decision, (3) G0 spike report → SPEC-LANGUAGE.md draft + ten target programs
- Last review gate: none yet (G0 pending)
