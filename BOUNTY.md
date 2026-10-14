# 💰 The Hybrid Bounty

We pay for **real soundness bugs** and **useful improvements** in Hybrid.
The pool is **$500** per cycle, split across the **top 10 most meaningful
contributions**. Open to humans and AI: we judge the report, not the author.

The maintainer keeps the final say on whether a report qualifies, its tier,
and its payout. Every decision gets a written reason in the issue.

## The ladder

| Place | Payout |
|-------|--------|
| 1st | **$75** |
| 2nd | **$60** |
| 3rd | **$50** |
| 4th to 10th | **$45 each** |

Total: $500. The top three take a larger share so the ladder stays
meaningful; exact amounts may be tuned before a cycle closes. Winners are
announced in the release notes and listed in the Hall of Fame below.

## What qualifies

Two tracks, ranked together:

1. **Soundness bugs (the main track).** A program Hybrid **accepts** but
   that is **unsound**: a data race on shared mutable state crossing a task
   boundary, a managed block escaping its contract, a stack-budget lie in a
   bridge call, a sendability proof granted without grounds. The bar is
   [GUARANTEES.md](GUARANTEES.md): an accepted-but-unsound program is a P0
   by definition.
2. **Useful improvements.** Fixes, adversarial corpus tests, diagnostic
   wording, docs, tooling. Meaningful beats large; a small fix with a test
   outranks a big idea with no repro.

## Severity tiers

| Tier | Meaning | Example |
|------|---------|---------|
| **Critical** | Silent data race, memory unsafety, or wrong results in accepted code | shared mutable cell read from two tasks with no proof |
| **High** | Soundness hole with narrower reach | an escape hatch skips a boundary check |
| **Medium** | A guarantee weaker than documented, or a misleading pass | sendability accepted on an unproven path |
| **Low** | Rough edges: diagnostics, docs, tooling gaps | confusing error text, missing repro hint |

Tier shapes ranking: a reproduced Critical outranks a nice-to-have.

## Payment rules, plain and short

- **Pay only after reproduction.** A soundness report must show a real
  accepted-but-unsound regression against our corpus: the compiler accepts
  it today, with steps, the exact commit, and a minimal `.hy` repro.
- **No payment for theories, duplicates, already-known issues, or anything
  we cannot reproduce.** The known-issues list lives in
  [GUARANTEES.md](GUARANTEES.md) and the issue tracker.
- **CI must pass on our side.** A fix PR has to go green on our GitHub
  Actions run at least once before it gets the final review that decides a
  merge. We never merge on local-only evidence.
- **Duplicates:** the first reporter takes the payout; later duplicates get
  Hall of Fame credit and a link to the original.
- One contribution takes one ladder slot. Ranking is by meaning, not size.

## How to submit

1. **Bugs:** open an issue with the
   [`soundness` template](https://github.com/limitlessprojects/Go-Hybrid/issues/new?template=soundness.yml).
   **Improvements:** a normal issue or a PR.
2. Include: what happens, what should happen, the exact commit, and the
   smallest repro (a `.hy` file or a test).
3. We aim to **acknowledge within 48 hours** with a first verdict:
   accepted, needs info, or duplicate, with reasons.
4. Fix PRs must pass CI on our side, then get final review.
5. Winners are announced when a cycle closes, in the release notes and here.

## Scope

**In scope:** the compiler crates (`compiler/crates/*`), the runtime Go
packages (`runtime/go/*`), the `hyb` CLI, the corpus, and the CI workflows
in this repo.

**Out of scope:** anything outside this repo, denial of service, bulk
automated scanning without a written report, social engineering, and
attacks on users or infrastructure.

## Safe harbor

Good-faith security research that follows this document is authorized, and
we will not pursue legal action over it. Stay in scope, use test accounts
and your own repos, and stop and report the moment you find something. When
in doubt, ask in
[Discussions](https://github.com/limitlessprojects/Go-Hybrid/discussions)
first.

## AI-assisted reports

AI-assisted hunting is welcome, with honest rules that keep the queue alive:

- Reports are judged on **verified impact**, never volume.
- Credit goes to the **GitHub account** that submits, whether a human or an
  AI operator stands behind it.
- Minimal repros only. No bulk dumps, no AI-generated fluff, no repeated
  near-duplicates.
- Flooding or spam disqualifies the account from the current cycle.

## Recognition beyond cash

- **Hall of Fame** (below) for every valid report and merged improvement.
- **Release notes shoutouts** for every winner.
- **All-contributors credit** on request: comment
  `@all-contributors please add @username for security` (or `bug`, `doc`,
  `code`) on your issue or PR and merge the bot's README update.
- Reliable reporters get invited to help triage, then earn wider repo roles.
  The ladder is: report, then triage help, then regular contributor, then
  trusted roles with ownership.

## Hall of Fame

| Who | What | When |
|-----|------|------|
| [ssmurfgg04-gif](https://github.com/ssmurfgg04-gif) | First community review: four substantive asks that shaped [GUARANTEES.md](GUARANTEES.md), the one-command install, and the `--deny-managed` hatch | 2026-10 |

Valid bug reports and merged improvements land here. Be next.

## A kind ask

If Hybrid interests you, **starring and sharing the repo helps more than
you think**. Bug reports make a project better, but stars and shares decide
whether anyone is around to use the fixes. If everyone reports faults and
nobody signals the project exists, the fixes die quiet. It costs nothing,
and it brings the next contributor.

## Community

We are **working on a Discord**; it will be linked here the day it opens.
Until then [Discussions](https://github.com/limitlessprojects/Go-Hybrid/discussions)
is the town hall: questions, ideas, show-and-tell, and bounty talk all land
there, and we answer every thread.
