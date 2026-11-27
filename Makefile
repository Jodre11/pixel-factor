# Every command an agent or contributor needs. Run `make help` for the list.

# Lowest line coverage `make coverage` accepts, in percent, for each crate.
# Core measured at 98.4%; service at 95.4%, without main.rs, which `make smoke`
# exercises instead.
COVERAGE_MIN ?= 98
SERVICE_COVERAGE_MIN ?= 95
# How long `make fuzz` runs, in seconds.
FUZZ_SECONDS ?= 60
# Parallel jobs for `make mutants`.
MUTANTS_JOBS ?= 2
# Settings for `make serve`. All development values: the pepper is public.
PF_LISTEN ?= 127.0.0.1:0
PF_DB_PATH ?= target/serve.db
PF_API_TOKEN ?= dev-token
PF_IMAGES ?= demo=0.75
PF_DEV_PEPPER_HEX ?= 6465762d6f6e6c792d7065707065722d6e6f742d666f722d70726f64756374696f6e
# Tag for `make image`, named after this checkout's directory so sessions in different worktrees do not
# overwrite each other's image. Characters a tag cannot hold become '-'.
IMAGE ?= pixel-factor-service:$(shell basename "$(CURDIR)" | tr -c 'A-Za-z0-9_.\n-' '-' | cut -c1-128)

.PHONY: help setup tools-deep check check-changed fmt fmt-check lint test doc \
	docs-check deny shellcheck fuzz-check advisories coverage mutants fuzz kat timing serve smoke image container-smoke iac-scan image-scan

help: ## List the available commands
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | sed -E 's/:.*## /\t/' | sort

setup: ## Prepare a fresh checkout: git hooks, dependencies, analysis tools
	@if git rev-parse --git-dir >/dev/null 2>&1; then git config core.hooksPath .githooks; fi
	cargo fetch --locked
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
	cargo test --workspace --exclude pixel-factor-fuzz --locked --offline

doc: ## Build the API docs with warnings as errors
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --exclude pixel-factor-fuzz --no-deps --locked --offline

docs-check: ## Fail if the docs name a command or file that does not exist
	@scripts/check-docs.sh

deny: ## Dependency policy: licences, sources, banned and duplicate crates
	cargo deny --offline check bans licenses sources

shellcheck: ## Lint the shell scripts
	shellcheck scripts/*.sh .githooks/pre-commit

fuzz-check: ## Compile the fuzz target, so it cannot rot
	cargo check -p pixel-factor-fuzz --locked --offline

advisories: ## Check dependencies against the RustSec advisory database (needs network)
	cargo deny check advisories

coverage: ## Line coverage per crate, failing below COVERAGE_MIN (core) or SERVICE_COVERAGE_MIN
	# A separate target directory per crate: cargo-llvm-cov reads every test binary in its directory, and a stale
	# service binary built against an older core would be counted as an unexecuted second copy of the core.
	CARGO_LLVM_COV_TARGET_DIR=target/llvm-cov-core \
		cargo llvm-cov --package pixel-factor-core --locked --offline --fail-under-lines $(COVERAGE_MIN)
	CARGO_LLVM_COV_TARGET_DIR=target/llvm-cov-service cargo llvm-cov --package pixel-factor-service --locked --offline \
		--ignore-filename-regex 'service/src/main\.rs' --fail-under-lines $(SERVICE_COVERAGE_MIN)

mutants: ## Mutation testing: break the code on purpose and see if a test notices (slow)
	cargo mutants --package pixel-factor-core --no-shuffle --jobs $(MUTANTS_JOBS) -- --locked --offline

fuzz: ## Fuzz `verify` for FUZZ_SECONDS seconds (needs a nightly toolchain)
	cargo +nightly fuzz run verify -- -max_total_time=$(FUZZ_SECONDS)

kat: ## Print the known-answer record (compare with core/tests/vectors/kat2.json)
	@cargo run --quiet --locked -p pixel-factor-core --example kat

serve: ## Run the HTTP service locally with development settings; prints its address
	PF_LISTEN=$(PF_LISTEN) PF_DB_PATH=$(PF_DB_PATH) PF_API_TOKEN=$(PF_API_TOKEN) PF_IMAGES=$(PF_IMAGES) \
		PF_DEV_PEPPER_HEX=$(PF_DEV_PEPPER_HEX) PF_BUILD_COMMIT=$(shell git rev-parse --short HEAD 2>/dev/null) \
		cargo run --quiet --locked --release -p pixel-factor-service

smoke: ## Start the service, drive enrol and verify through HTTP, then stop it
	@scripts/smoke.sh

image: ## Build the service container image as IMAGE
	docker build --build-arg PF_BUILD_COMMIT=$(shell git rev-parse --short HEAD 2>/dev/null) -t $(IMAGE) .

container-smoke: ## Run IMAGE with development settings and drive it over HTTP (run `make image` first)
	scripts/container-smoke.sh $(IMAGE)

iac-scan: ## Trivy misconfiguration scan of the Dockerfile and workflows
	trivy config --exit-code 1 --severity HIGH,CRITICAL .

image-scan: ## Trivy vulnerability scan of IMAGE (run `make image` first)
	trivy image --exit-code 1 --severity HIGH,CRITICAL --ignore-unfixed $(IMAGE)

timing: ## Time one enrolment and verification at production settings
	@cargo run --quiet --locked --release -p pixel-factor-core --example timing
