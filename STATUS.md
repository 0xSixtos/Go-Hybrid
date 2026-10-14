# STATUS (for the human — 15 lines max)

- Phase: **MVP DONE + v0.1.0 released + community OPEN** — G0→G3 full gate chain green; customer feedback landing in batches (Batch M fail-closed, Batch N trust-blockers)
- Community: **Discussion #1 got its first substantive review** (4 asks: evidence table, install friction, adoption framing, hatch hardening) — ALL FOUR merged same-day: GUARANTEES.md claims table · one-command `install.sh` + Rust-free tarball · `--deny-managed` CI gate · rows 11–15 opened for the rest (platform matrix, Docker, go.mod sample repo, managed reason/categories, soundness ledger)
- The catch this batch: **v0.1.0's tarball had no prebuilt staticlib** — INSTALL.md asked users to install Rust 1.99.0, exactly the pitch-death the reviewer predicted ("if I have to build Rust first, I'll wait"). Fixed: `tools/package_release.sh` rebuilds the tarball with the prebuilt `libhyrt.a` (mtime-freshness contract) and smoke-tests the REAL tarball with cargo REMOVED from PATH — rustless PASS; re-uploaded to the release
- Install bar now: one command (`install.sh`), no Rust, sha256-verified, launcher wires HYB_RUNTIME_DIR, verified via `hyb explain E0002` (never `--help` — exit 2 by design)
- Works (verified this session): cargo **210/0** (+4 deny-managed integration tests) · corpus **20/20** · differential **9/9+11/11** · examples **10/10** · go vet+build+test clean · clippy 0 · packaging smoke rustless PASS
- Queue: rows 1-3, 5, 7-10 closed · **open**: 4 (per-callee budgets), 6 (make bench at next fleet dispatch), 11 (multi-platform matrix), 12 (Docker), 13 (go.mod sample repo design), 14 (managed reason+categories), 15 (soundness ledger+bounty)
- Next 3 actions: (1) discussion reply + new threads (platform poll, go.mod design question), (2) row 4 per-callee budgets design, (3) release matrix at the next fleet dispatch (row 6 rides it)
- Last review gate: G3 PASSED 2026-10-13 (full chain green); community review is now a standing input to the queue
