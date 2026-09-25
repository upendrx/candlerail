.PHONY: help build test lint fmt run templates

help: ## Show this list
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "} {printf "  %-10s %s\n", $$1, $$2}'

build: ## Release build
	cargo build --release

test: ## All tests
	cargo test --workspace

lint: ## rustfmt check and clippy with warnings as errors
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings

fmt: ## Format the code
	cargo fmt --all

run: build ## Start the web app on http://127.0.0.1:8787, serving ui/ from disk
	./target/release/candlerail serve --ui-dir ui

templates: build ## Validate and explain every template
	@for t in $$(./target/release/candlerail templates | awk 'NF==0{exit} {print $$1}'); do ./target/release/candlerail explain $$t > /dev/null && echo "ok  $$t"; done
