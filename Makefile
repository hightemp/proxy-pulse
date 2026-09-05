.PHONY: help doctor install-deps dev preview build build-debug package test test-integration test-ui test-native lint format typecheck quality clean

help: ## Show available commands
	@python3 scripts/doctor.py --help-targets

doctor: ## Check the local development environment
	python3 scripts/doctor.py

install-deps: ## Install locked frontend dependencies
	pnpm install --frozen-lockfile

dev: ## Launch the native desktop app in development mode
	pnpm desktop

preview: ## Open a frontend-only development server
	pnpm dev

build: ## Build the release desktop executable
	pnpm tauri build --no-bundle

build-debug: ## Build a standalone debug desktop executable
	pnpm tauri build --debug --no-bundle

package: ## Build a native installer (Linux default: Debian package)
	pnpm tauri build --bundles deb

test: ## Run core contract and property tests
	cargo test -p proxy-pulse-core --locked

test-integration: ## Check real protocols against local proxy fixtures
	cargo build -p proxy-pulse-core --example check --locked
	python3 scripts/network_fixtures.py

test-ui: ## Run browser layout and accessibility smoke checks
	python3 scripts/run_ui_tests.py

test-native: ## Run the real Tauri GUI smoke (requires a running tauri-driver)
	python3 scripts/native_smoke.py

lint: ## Check Rust and TypeScript without warnings
	cargo clippy --workspace --all-targets --locked -- -D warnings
	pnpm typecheck

format: ## Format Rust and frontend sources
	cargo fmt --all
	pnpm exec prettier --write src tests-ui *.json *.ts index.html src-tauri/*.json src-tauri/capabilities/*.json

typecheck: ## Check frontend types
	pnpm typecheck

quality: ## Run format, static, contract and protocol checks
	pnpm format:check
	$(MAKE) lint test test-integration test-ui

clean: ## Remove only generated build and test output
	cargo clean
	python3 scripts/clean.py
