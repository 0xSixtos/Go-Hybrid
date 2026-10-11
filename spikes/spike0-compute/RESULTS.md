# Spike 0: Compute smoke tests (Kaggle worker + GitHub Actions fleet)

**Status:** hypothesis pre-registered 2026-10-11 (before any run; per SPEC §14.7 "pre-register thresholds").
**Job:** `jobs/001-spike0-kaggle-smoke` (Kaggle) and `.github/workflows/heavy.yml` dispatch `spike0` (Actions).

## Hypothesis

The two remote tiers (Kaggle worker via API, GitHub Actions standard runners) are usable for
the compute roles SPEC §2 assigns them: Kaggle = one-big-machine jobs, Actions = embarrassingly
parallel shard fleet. Unverified specifics we expect to CORRECT: real Kaggle core count (SPEC
says "reportedly 16"), session limits, internet/secret availability in API-launched runs, and
real Actions runner hardware (reportedly 4 vCPU/16 GB public, 2 vCPU/7 GB private — repo is
public).

## Pass/fail thresholds (pre-registered)

**Part A — Kaggle worker (job 001):**
| # | Metric | PASS | FAIL |
|---|--------|------|------|
| A1 | End-to-end: job submitted→results fetched | completes | unusable after 2 retries |
| A2 | Wall clock (queue + install + run) | < 45 min | > 90 min |
| A3 | Pinned Go + Rust install (minimal) | both build hello-world | either fails |
| A4 | Internet from API-launched run | repo clone at SHA works | blocked |
| A5 | Reported cores / RAM / disk | recorded whatever is true | n/a (measurement) |

Decision table (from SPEC §5 Spike 0): works→adopt two-tier protocol; no internet→source via
private dataset; high friction→batch-only; unusable→Actions + sandbox-only fallback.

**Part B — Actions fleet (workflow `heavy`, dispatch `spike0`):**
| # | Metric | PASS | FAIL |
|---|--------|------|------|
| B1 | 4-shard matrix runs + reduce merges summary.json | yes | no |
| B2 | 20-shard run: measured concurrency | ≥ 16 concurrent | < 8 |
| B3 | Per-shard: pinned toolchains + Go/Rust hello-world | all green | any red |
| B4 | Artifact size per shard | < 1 MB | > 10 MB |
| B5 | Queue latency per shard (p50) | < 3 min | > 10 min |

Decision table: 20 concurrent + clean merge→adopt as written; fewer→lower default max-parallel;
minutes tight→(N/A: repo public, standard runners free); unusable→Kaggle + sandbox fallback.

## Method

- One generic runner (`tools/runner/runner.py`, stdlib-only Python) executes `job.json` on all
  tiers; Kaggle kernel (`worker/script.py`) and the Actions workflow are thin wrappers.
- Jobs always run a pinned commit SHA of this public repo (no secrets leave the sandbox; the
  worker clones anonymously).
- Kaggle: `kaggle kernels push` (private kernel, internet on, GPU off), poll with backoff,
  `kaggle kernels output` fetch.
- Actions: `workflow_dispatch` with `sha` + `shards` inputs; shards assigned deterministically
  (shard id = i % n); `reduce` job merges `summary.json` fragments.

## Results

(filled after the run — raw numbers only, decisions go to PLAN.md)

### Part A — Kaggle (job 001)
- Run at SHA: <fill>
- Submitted (UTC): <fill> → completed: <fill> → wall clock: <fill>
- Queue latency / startup: <fill>
- Toolchain install: go <fill> min, rust <fill> min
- Environment: nproc=<fill>, RAM=<fill>, disk=<fill>, cgroup limits=<fill>
- Internet: <fill>; preexisting toolchains: <fill>
- Hello-world Go: <pass/fail>; Rust: <pass/fail>
- Output size seen / limit hit: <fill>
- Session limit observed: <fill>

### Part B — Actions (workflow heavy / spike0)
- Run at SHA: <fill> (run id <fill>)
- 4-shard: queue p50 <fill>, concurrency achieved <fill>, merge <fill>, minutes/shard <fill>
- 20-shard: queue p50 <fill>, concurrency achieved <fill>, merge <fill>, minutes/shard <fill>
- Runner hardware: <fill> (from env report)
- Artifacts: size/shard <fill>, retention behavior <fill>

## Verdict

(fill after both parts: which decision-table row fired)
