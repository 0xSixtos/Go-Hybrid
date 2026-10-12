# QUESTIONS.md — escalations to the human

Protocol (SPEC.md §9): for credentials/destructive actions/weeks-costly-wrong/legal/budget items only.
Each entry: context, options, our recommendation, and what we do in the meantime.
Design choices are decided autonomously per the decision rules and logged in PLAN.md.

## Open questions

### Q1 — Tier-0 sendability reading (gate G0 decision; blocks the SPEC-LANGUAGE.md race-prevention claim)

**Context.** Spike 6 confirmed the ten MVP programs need zero annotations
under every reading of the sendability rule (A1), so the corpus cannot pick
a reading. The reading decides whether Tier 0 *promises* data-race
prevention for mutable GC state:

- (a) **Go-compatible** — sendable, sender keeps using it; races remain
  possible (Go's happens-before story). Zero friction, weakest claim.
- (b) **Freeze-after-send** — sendable, but the sender's subsequent writes
  through live aliases are compile errors. Medium friction, strong claim.
- (c) **Safe-by-type** — sendable only if channel/atomic/immutable, or an
  owned move, or `managed`. Strongest claim, most friction (Go-idiomatic
  code that shares a map across tasks needs `managed`).

**Recommendation.** (c)+(b) for v1 (strictest — safety claims are the
project's reason to exist; `managed` is the documented escape hatch, and
Spike 6 shows the ten target programs never need it). Add the three
adversarial corpus programs before the checker lands so the choice is
test-pinned.

**Meanwhile.** Writing SPEC-LANGUAGE.md with (c)+(b) as the default,
clearly marked "provisional pending human review" (G0 default-per-SPEC §9).

### Q2 — Thread budget: derived-by-probe vs configured-by-default

**Context.** The bridge's hard thread budget gates the syswin lane. The
sandbox walls at ~717 threads (address space); the Kaggle worker allows
38,508 pids and ran 5,262 threads happily. A fixed default is wrong on one
of the two tiers by ~50×.

**Options.** (a) derive-by-probe (the 50 ms sampler — current design);
(b) configurable default + probe as fallback; (c) fixed conservative
default (e.g. 512) everywhere.

**Recommendation.** (b): ship a conservative default (512), allow
configuration, keep the sampler as the enforcement mechanism regardless —
probing then only *lowers* the effective budget, never raises it above the
configured ceiling.

**Meanwhile.** (a) stands in the spike code; the descriptor schema records
both fields.

## Resolved questions

(none yet)
