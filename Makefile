# open-runo development & quality-gate entrypoints.
# Mirrors the commands documented in CONTRIBUTING.md / DEVELOPMENT.md so
# local runs and CI always exercise the exact same steps.

CARGO ?= cargo

.PHONY: help build release test fmt fmt-check clippy audit deny doc \
        run run-router clean quality-gate ci pre-commit

help:
	@echo "open-runo make targets:"
	@echo "  make build          - cargo build (debug, all workspace members)"
	@echo "  make release        - cargo build --release"
	@echo "  make test           - cargo test --workspace --all-features"
	@echo "  make fmt            - cargo fmt (auto-format)"
	@echo "  make fmt-check      - cargo fmt --check (CI mode, no changes written)"
	@echo "  make clippy         - cargo clippy --all-targets --all-features -- -D warnings"
	@echo "  make audit          - cargo audit (known-vulnerability scan)"
	@echo "  make deny           - cargo deny check (license + advisory + ban policy)"
	@echo "  make doc            - cargo doc --workspace --no-deps"
	@echo "  make run-router     - run the open-runo-router gateway locally"
	@echo "  make quality-gate   - fmt-check + clippy + test + audit + deny (full CI gate)"
	@echo "  make pre-commit     - fmt + clippy + test (fast local pre-commit loop)"
	@echo "  make clean          - cargo clean"

build:
	$(CARGO) build --workspace --all-features

release:
	$(CARGO) build --workspace --all-features --release

test:
	$(CARGO) test --workspace --all-features

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all --check

clippy:
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

audit:
	$(CARGO) audit

deny:
	$(CARGO) deny check

doc:
	$(CARGO) doc --workspace --all-features --no-deps

run-router:
	$(CARGO) run -p open-runo-router

clean:
	$(CARGO) clean

# Full gate: what CI runs on every PR. Keep in sync with
# .github/workflows/ci.yml's `quality-gate` job.
quality-gate: fmt-check clippy test audit deny
	@echo "Quality gate passed."

# Fast local loop before committing (skips audit/deny, which need network
# access / advisory DB updates).
pre-commit: fmt clippy test
	@echo "Pre-commit checks passed."
