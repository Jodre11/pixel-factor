# Every command an agent or contributor needs. Run `make help` for the list.

# Lowest line coverage `make coverage` accepts, in percent. Measured at 98.68%.
COVERAGE_MIN ?= 98
# How long `make fuzz` runs, in seconds.
FUZZ_SECONDS ?= 60
# Parallel jobs for `make mutants`.
MUTANTS_JOBS ?= 2

.PHONY: help setup tools-deep check check-changed fmt fmt-check lint test doc \
	docs-check deny shellcheck fuzz-check advisories coverage mutants fuzz kat timing

help: ## List the available commands
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | sed -E 's/:.*## /\t/' | sort

setup: ## Prepare a fresh checkout: git hooks, dependencies, analysis tools
	@if git rev-parse --git-dir >/dev/null 2>&1; then git config core.hooksPath .githooks; fi
	cargo fetch --locked
	cargo fetch --locked --manifest-path fuzz/Cargo.toml
	@scripts/install-tools.sh

tools-deep: ## Install the tools for coverage, mutation testing and fuzzing
	@scripts/install-tools.sh deep

check: fmt-check lint test doc docs-check deny shellcheck fuzz-check ## Prove a change is sound. This is what CI runs

check-changed: ## Check only what changed against main (or pass BASE=<ref>)
	@scripts/check-changed.sh $(BASE)

fmt: ## Format the code
	cargo fmt --all

fmt-check: ## Fail if the code is not formatted
	cargo fmt --all --check

lint: ## Clippy with warnings as errors
	cargo clippy --workspace --all-targets --locked --offline -- -D warnings

test: ## Run every test, including property tests, with no network access
	cargo test --workspace --locked --offline

doc: ## Build the API docs with warnings as errors
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline

docs-check: ## Fail if the docs name a command or file that does not exist
	@scripts/check-docs.sh

deny: ## Dependency policy: licences, sources, banned and duplicate crates
	cargo deny --offline check bans licenses sources

shellcheck: ## Lint the shell scripts
	shellcheck scripts/*.sh .githooks/pre-commit

fuzz-check: ## Compile the fuzz target, so it cannot rot
	cargo check --manifest-path fuzz/Cargo.toml --locked --offline

advisories: ## Check dependencies against the RustSec advisory database (needs network)
	cargo deny check advisories

coverage: ## Line coverage, failing below COVERAGE_MIN percent
	cargo llvm-cov --workspace --locked --offline --fail-under-lines $(COVERAGE_MIN)

mutants: ## Mutation testing: break the code on purpose and see if a test notices (slow)
	cargo mutants --package pixel-factor-core --no-shuffle --jobs $(MUTANTS_JOBS) -- --locked --offline

fuzz: ## Fuzz `verify` for FUZZ_SECONDS seconds (needs a nightly toolchain)
	cargo +nightly fuzz run verify -- -max_total_time=$(FUZZ_SECONDS)

kat: ## Print the known-answer record (compare with core/tests/vectors/kat1.json)
	@cargo run --quiet --locked -p pixel-factor-core --example kat

timing: ## Time one enrolment and verification at production settings
	@cargo run --quiet --locked --release -p pixel-factor-core --example timing
