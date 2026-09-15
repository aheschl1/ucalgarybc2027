.PHONY: all sync build dev sdk test test-rust test-py lint fmt clean

build:
	cargo build --workspace

sdk:
	cargo run -q -p ucbc-cli -- gen-sdk ucbc-sdk/ucbc/games

dev: sdk
	uv run maturin develop --uv

test: test-rust test-py

test-rust:
	cargo test --workspace

test-py: dev
	uv run pytest

lint:
	cargo run -q -p ucbc-cli -- gen-sdk ucbc-sdk/ucbc/games --check
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	uv run ruff check ucbc-sdk bots
	uv run ruff format --check ucbc-sdk bots
	uv run mypy

clean:
	cargo clean
	rm -rf .venv dist
