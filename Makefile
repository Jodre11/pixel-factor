# Every command an agent or contributor needs. Run `make help` for the list.

.PHONY: help setup check check-changed fmt fmt-check lint test docs-check kat timing

help: ## List the available commands
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | sed -E 's/:.*## /\t/' | sort

setup: ## Prepare a fresh checkout: git hooks and dependencies
	@if git rev-parse --git-dir >/dev/null 2>&1; then git config core.hooksPath .githooks; fi
	cargo fetch --locked

check: fmt-check lint test docs-check ## Prove a change is sound: format, lint, tests, docs

check-changed: ## Check only what changed against main (or pass BASE=<ref>)
	@scripts/check-changed.sh $(BASE)

fmt: ## Format the code
	cargo fmt --all

fmt-check: ## Fail if the code is not formatted
	cargo fmt --all --check

lint: ## Clippy with warnings as errors
	cargo clippy --workspace --all-targets --locked -- -D warnings

test: ## Run every test, with no network access
	cargo test --workspace --locked --offline

docs-check: ## Fail if the docs name a command or file that does not exist
	@scripts/check-docs.sh

kat: ## Print the known-answer record (compare with core/tests/vectors/kat1.json)
	@cargo run --quiet --locked -p pixel-factor-core --example kat

timing: ## Time one enrolment and verification at production settings
	@cargo run --quiet --locked --release -p pixel-factor-core --example timing
