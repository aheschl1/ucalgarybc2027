.PHONY: all sync build dev sdk viewer-types viewer wheels test test-rust test-py test-viewer lint fmt clean

# `make dev GAME=tictactoe` or `make wheels GAME=tictactoe` builds one game only:
# the engine with that cargo feature, the SDK with that game's handle. Unset: all.
GAME ?=
FEATURES := $(if $(GAME),--no-default-features -F $(GAME),)

build:
	cargo build --workspace

sdk:
	cargo run -q -p ucbc-cli -- gen-sdk ucbc-sdk/ucbc/games

# TypeScript types for the viewer, from the replay schema and each game's API.
viewer-types:
	npm ci
	node ucbc-viewer/scripts/gen-types.mjs

# The viewer page, built into the ucbc_engine package for `ucbc view`.
viewer:
	npm ci
	npm run build -w @ucbc/viewer -- --outDir ../ucbc-py/python/ucbc_engine/viewer/static --emptyOutDir

dev: sdk
	cd ucbc-py && uv run maturin develop --uv $(FEATURES)

# Release wheels into dist/: ucbc (engine, runtime, CLI) and ucbc-sdk.
wheels: sdk viewer
	rm -rf dist && mkdir -p dist
	uv build --package ucbc --wheel -o dist -C build-args="$(FEATURES)"
ifeq ($(GAME),)
	uv build --package ucbc-sdk --wheel -o dist
else
	tmp=$$(mktemp -d) && cp -r ucbc-sdk $$tmp/sdk \
	  && find $$tmp/sdk/ucbc/games -mindepth 1 -maxdepth 1 -type d ! -name $(GAME) -exec rm -rf {} + \
	  && uv build --wheel -o dist $$tmp/sdk && rm -rf $$tmp
endif

test: test-rust test-py test-viewer

test-rust:
	cargo test --workspace

test-py: dev
	uv run pytest

test-viewer:
	npm ci
	npm test --workspaces

lint:
	cargo run -q -p ucbc-cli -- gen-sdk ucbc-sdk/ucbc/games --check
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	uv run ruff check ucbc-sdk ucbc-py/python tests bots
	uv run ruff format --check ucbc-sdk ucbc-py/python tests bots
	uv run mypy
	node ucbc-viewer/scripts/gen-types.mjs --check
	npm run typecheck --workspaces

clean:
	cargo clean
	rm -rf .venv dist node_modules
