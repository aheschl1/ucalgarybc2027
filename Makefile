.PHONY: build sdk dev wheels test test-rust test-py test-api lint clean up down db migrate api

# `make dev GAME=tictactoe` or `make wheels GAME=tictactoe` builds one game only:
# the engine with that cargo feature, the SDK with that game's handle. Unset: all.
GAME ?=
FEATURES := $(if $(GAME),--no-default-features -F $(GAME),)

build:
	cargo build --workspace

sdk:
	cargo run -q -p ucbc-cli -- gen-sdk ucbc-sdk/ucbc/games

dev: sdk
	cd ucbc-py && uv run maturin develop --uv $(FEATURES)

# Release wheels into dist/: ucbc (engine, runtime, CLI) and ucbc-sdk.
wheels: sdk
	rm -rf dist && mkdir -p dist
	uv build --package ucbc --wheel -o dist -C build-args="$(FEATURES)"
ifeq ($(GAME),)
	uv build --package ucbc-sdk --wheel -o dist
else
	tmp=$$(mktemp -d) && cp -r ucbc-sdk $$tmp/sdk \
	  && find $$tmp/sdk/ucbc/games -mindepth 1 -maxdepth 1 -type d ! -name $(GAME) -exec rm -rf {} + \
	  && uv build --wheel -o dist $$tmp/sdk && rm -rf $$tmp
endif

test: test-rust test-py test-api

test-rust:
	cargo test --workspace

test-py: dev
	uv run pytest --ignore=tests/api

# Starts its own Postgres through testcontainers; needs Docker.
test-api: dev
	uv run pytest tests/api

lint:
	cargo run -q -p ucbc-cli -- gen-sdk ucbc-sdk/ucbc/games --check
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	uv run ruff check ucbc-sdk ucbc-py/python ucbc-api alembic tests bots
	uv run ruff format --check ucbc-sdk ucbc-py/python ucbc-api alembic tests bots
	uv run mypy

clean:
	cargo clean
	rm -rf .venv dist

# Platform: Postgres and the API. `make up` builds the API image, migrates, and serves on :8000.
# `ENV=prod make up` substitutes .env.prod into compose instead of .env.local.
ENV ?= local
COMPOSE := docker compose --env-file .env --env-file .env.$(ENV)

up:
	$(COMPOSE) up --build

down:
	$(COMPOSE) down

db:
	$(COMPOSE) up -d --wait db

migrate:
	uv run alembic upgrade head

api:
	uv run ucbc-api
