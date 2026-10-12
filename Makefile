# Go-Hybrid — one-command routine actions (SPEC.md §14.1)
# NOTE: `just` was deferred (PLAN.md Decision Log); make is ubiquitous on this box.
SHELL := /bin/sh
PY := python3
KAGGLE_CLI ?= /home/z/.venv/bin/kaggle
JOB ?= 001-spike0-kaggle-smoke
SHARDS ?= 4
MAXPAR ?= 4

.PHONY: help env check test-quick ci corpus workspace spike0-kaggle spike0-kaggle-status spike0-kaggle-fetch \
	spike0-actions-4 spike0-actions-20 spike-1 spike-2 spike-3 kaggle-job kaggle-status kaggle-fetch kaggle-wait \
	actions-dispatch actions-watch actions-fetch clean

CARGO := $(HOME)/.cargo/bin/cargo
CARGO_TARGET_DIR ?= $(HOME)/go-hybrid-target
HYB_BIN := $(CARGO_TARGET_DIR)/debug/hyb

help:
	@echo "make env              — verify pinned toolchains (source tools/env.sh)"
	@echo "make check            — quick sanity: tool syntax + runner selftest + hygiene"
	@echo "make test-quick       — sandbox unit tests (grows with the compiler)"
	@echo "make ci               — what CI runs on push"
	@echo "make spike0-kaggle    — submit Spike 0 part A to the Kaggle worker"
	@echo "make spike0-kaggle-status / -fetch"
	@echo "make spike0-actions-4 / -20  — dispatch the Actions fleet smoke test"
	@echo "make spike-1 / spike-2 / spike-3 — re-run that spike's bench suite (sandbox)"
	@echo "make kaggle-job J=NNN-name   — submit jobs/\$(J) to the worker"
	@echo "make clean            — prune caches (disk hygiene)"

env:
	@tools/env.sh

check:
	@$(PY) -m py_compile tools/runner/runner.py tools/job.py worker/script.py tools/corpus_harness.py && echo "py-syntax OK"
	@cd /tmp && rm -rf gh-selftest && mkdir gh-selftest && cd gh-selftest && \
	printf '%s\n' '{"job":"selftest","tier":"sandbox","sha":"none","repo":"none","gpu":false,"setup":[],"commands":[{"name":"echo","run":"echo selftest-ok","timeout":30}],"expected_results":[]}' > job.json && \
	$(PY) $(CURDIR)/tools/runner/runner.py job.json --results-dir results >/dev/null && \
	test -f results/summary.json && grep -q '"ok": true' results/summary.json && echo "runner selftest OK"
	@$(PY) -c "import glob,json;[json.load(open(p)) for p in glob.glob('jobs/*/job.json')];print('job specs OK')"
	@! git grep -qE 'ghp_[A-Za-z0-9]{20,}|KGAT_[A-Za-z0-9]{20,}|eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,}' HEAD 2>/dev/null || (echo "SECRET LEAK"; exit 1)
	@cd compiler && CARGO_TARGET_DIR=$(CARGO_TARGET_DIR) CARGO_BUILD_JOBS=2 $(CARGO) check --workspace --quiet && echo "cargo check OK"
	@echo "check OK"

test-quick: check
	@echo "(no compiler tests yet — Phase 0; see PLAN.md)"

CARGO := $(HOME)/.cargo/bin/cargo
CARGO_TARGET_DIR ?= $(HOME)/go-hybrid-target
HYB_BIN := $(CARGO_TARGET_DIR)/debug/hyb

workspace:
	@cd compiler && CARGO_TARGET_DIR=$(CARGO_TARGET_DIR) CARGO_BUILD_JOBS=2 $(CARGO) check --workspace

corpus:
	@cd compiler && CARGO_TARGET_DIR=$(CARGO_TARGET_DIR) CARGO_BUILD_JOBS=2 $(CARGO) build -p hyb --quiet
	@$(PY) tools/corpus_harness.py --bin $(HYB_BIN)

ci: check
	@echo "CI parity with .github/workflows/ci.yml"

# ---- Spike 0 orchestration ----
spike0-kaggle:
	$(PY) tools/job.py kaggle submit jobs/$(JOB)

spike0-kaggle-status:
	$(PY) tools/job.py kaggle status

spike0-kaggle-fetch:
	$(PY) tools/job.py kaggle fetch jobs/$(JOB)

spike0-actions-4:
	$(PY) tools/job.py actions dispatch heavy "$(shell git rev-parse --short=40 HEAD)" $(JOB) 4 4

spike0-actions-20:
	$(PY) tools/job.py actions dispatch heavy "$(shell git rev-parse --short=40 HEAD)" $(JOB) 20 20

kaggle-job:
	$(PY) tools/job.py kaggle submit jobs/$(J)

kaggle-status:
	$(PY) tools/job.py kaggle status

kaggle-wait:
	$(PY) tools/job.py kaggle wait $(or $(MINUTES),20)

kaggle-fetch:
	$(PY) tools/job.py kaggle fetch jobs/$(J)

spike-1:
	bash spikes/spike1-call-overhead/work/run.sh

spike-2:
	bash spikes/spike2-go-linking/work/run.sh

spike-3:
	bash spikes/spike3-memory/work/run.sh

spike-5:
	bash spikes/spike5-errors/work/run.sh

spike-4:
	bash spikes/spike4-scheduling/work/run.sh

actions-watch:
	$(PY) tools/job.py watch actions

clean:
	@command -v cargo >/dev/null && cargo clean 2>/dev/null || true
	@if command -v go >/dev/null; then GOFLAGS= go clean -cache 2>/dev/null || true; fi
	@df -h / | tail -1
