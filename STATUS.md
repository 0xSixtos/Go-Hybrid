# STATUS (for the human — 15 lines max)

- Phase: **MVP DONE + v0.1.0 released + community OPEN** — G0→G3 full gate chain green; customer feedback landing in batches (Batch M fail-closed, Batch N trust-blockers, Batch Q arm64 port)
- Batch Q (row 16) DONE 2026-10-14: **the aarch64 bridge port landed** — call_arm64.s (full trampoline set, g-in-R28 no-TLS guard check, zero callee-saved clobber), arm64 offsets extracted from arm64 DWARF (stackguard0=16, toolchain cross-checked), and the FULL 5-package Go suite passes under qemu against an arm64 staticlib
- The bridge smoke the port added **caught three real release bugs, all fixed same-batch**: hyb couldn't find its own tarball runtime without the launcher (exe-relative discovery now); staticlib selection was arch-blind (ELF e_machine filter + proper ar walk now); the single-program path always cargo-rebuilt natively — reachable even in the rustless smoke via a ~/.cargo fallback (removed — rustless is real again)
- linux/arm64 now ships **FULL** (checker + emitter + bridge, both smokes green from the actual tarball); fleet dispatch from this SHA rebuilds + republishes all four assets with the new cut table
- Works (verified this session): cargo **212/0** (+2 arch-matcher tests) · corpus **20/20** · differential **9/9+11/11** · examples **10/10** · go vet+build+test amd64 **5 pkgs** + **5/5 under qemu (arm64)** · clippy 0 · packaging dual-smoke rustless PASS
- Community: discussion #1 (review) answered + merged same-day; #2 (platform poll) resolved by the fleet + updated; #3 (go.mod design) live, awaiting public argument before row 13 lands
- Queue: rows 1-7, 9-11, 15-16 CLOSED · **open: 12 (Docker image), 13 (go.mod sample repo — thread #3), 14 (managed reason+categories — design thread first)**
- Next 3 actions: (1) row 14 managed hardening design thread → implementation, (2) row 13 go.mod integration per thread #3's leaning, (3) row 12 Dockerfile + GHCR
- Last review gate: G3 PASSED 2026-10-13 (full chain green); community review is a standing input to the queue
