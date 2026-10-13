# STATUS (for the human — 15 lines max)

- Phase: 1 — **IN PROGRESS**. **G1 RATIFIED (2026-10-13)** — every lowering decision re-checked against SPEC (T4/E3/E6/C1–C4/M4/O8–O9) + implementation; no redirects; batch order confirmed (gates/G1.md header + PLAN Decision Log)
- **Phase-1 exit criteria MET (session 5): parse → Tier-0 check → emit Go → go build → run on SIX target programs — 01/02/03/04/07/08 all byte-exact vs expected.txt**
- Works (verified): example 02's full E6 `?`-exit contract (defers → `cat: open …` on stderr → exit 1) AND byte-verbatim happy path; 03's structs + task→go + channels; 04's select + inclusive ranges + labeled breaks; 07's generics + option/match + set/pop; 08's type aliases + O8 slot-loan desugar
- Works (verified): Tier-0 checker corpus **20/20 verdict agreement**, tripwire not fired (10.0%/15.0% vs ~25%); 157 compiler tests green; `hyb build` enforces toolchain pins (HYB-E9001); CI green
- Honest scope: 05/10 need the Rust bridge (Phase 2, NOW UNBLOCKED), 09 needs multi-package emission, 06 needs method-vs-field type knowledge — all error cleanly today, never wrong output
- Blocked: nothing
- Next 3 actions: (1) example 06 — receiver-hint typing for method-vs-field chains, (2) Phase-2 bridge → programs 05/09/10, (3) differential corpus loop + call-overhead bench as G2 evidence
- Last review gate: **G1 RATIFIED**
