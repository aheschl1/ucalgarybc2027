.PHONY: help setup build gen sdk viewer-types editor-types api-types viewer editor web replays test test-api lint clean up down db
.DEFAULT_GOAL := help

# Builds include ucbc-games' default games. The tests build every game
test test-api: FEATURES = -F ucbc-games/all

# `ENV=prod make up` substitutes .env.prod into compose instead of .env.local.
ENV ?= local
COMPOSE := docker compose --env-file .env --env-file .env.$(ENV)

PY := ucbc-cli/python ucbc-api ucbc-worker alembic tests bots

help:
	@grep -hE '^[a-z-]+:.*##' $(MAKEFILE_LIST) | sed 's/:.*##\s*/\t/' | column -ts '	' | sed 's/^/  /'

setup: .env .env.local build  ## once: env files, then everything `make build` does

# uv builds the engine, here and only here: a `uv run` afterwards leaves the build alone.
build: gen viewer editor runtime  ## after any change causing behavior changes in games: generated code, the viewer and editor pages, the bot runtime, dependencies and the engine in .venv
	MATURIN_PEP517_ARGS="$(FEATURES)" uv sync --all-packages --reinstall-package ucbc

test: .env .env.local build  ## rust, python, api (needs Docker), viewer
	cargo test --workspace
	uv run pytest
	npm test --workspaces --if-present

test-api: .env .env.local build
	uv run pytest tests/api

lint: gen  ## cargo fmt/clippy, ruff, mypy, tsc; regenerates first, so a stale checkout shows in git status
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	uv run ruff check $(PY)
	uv run ruff format --check $(PY)
	uv run mypy
	npm run typecheck --workspaces

up: .env .env.$(ENV)  ## the platform in Docker
	$(COMPOSE) up --build

down:  ## stop compose
	$(COMPOSE) down

# Postgres and MinIO alone
db: .env .env.$(ENV)  ## Postgres and MinIO in Docker, for running the API from the checkout
	$(COMPOSE) up -d --wait db minio

clean:  ## build outputs, .venv, node_modules, and the downloads and builds in .cache
	cargo clean
	rm -rf .venv dist node_modules ucbc-api/api/static $(RUNTIME) $(CACHE)


.env .env.local:
	cp $@.example $@

node_modules: package-lock.json
	npm ci
	touch $@

# Generated code:
#   sdk           Rust game types   -> Python SDK (ucbc-cli/python/ucbc/games)
#   viewer-types  Rust JSON Schemas -> viewer and renderer TypeScript (*.gen.ts)
#   editor-types  map.proto         -> map editor TypeScript
#   api-types     API OpenAPI       -> web app TypeScript (ucbc-web/src/api.gen.ts)
gen: sdk viewer-types editor-types api-types

sdk:
	cargo run -q -p ucbc-dev -- gen-sdk ucbc-cli/python/ucbc/games

viewer-types: node_modules
	node ucbc-viewer/scripts/gen-types.mjs

editor-types: node_modules
	npm run gen -w @ucbc/editor-ucbc2027 --silent

api-types: node_modules
	node ucbc-web/scripts/gen-api.mjs

viewer: viewer-types
	npm run build -w @ucbc/viewer -- --outDir ../ucbc-cli/python/ucbc/viewer/static --emptyOutDir

editor: editor-types
	npm run build -w @ucbc/editor-ucbc2027 -- --outDir ../../../ucbc-cli/python/ucbc/editor/static --emptyOutDir

REPLAY_MAPS := standard medium
replays: build  ## rewrite the viewer's sample replays, when tests say they are stale
	for map in $(REPLAY_MAPS); do \
		uv run ucbc run bots/ucbc2027/example bots/ucbc2027/example --no-verbose \
			--map games/ucbc2027/maps/$$map.map --replay games/ucbc2027/viewer/replays/$$map.json.gz > /dev/null; \
	done

web: viewer-types api-types
	npm run build -w ucbc-web -- --outDir ../ucbc-api/api/static --emptyOutDir

# The bot runtime (`make runtime`, which `build` needs), the guest interpreter and CPython
# for WASI behind it, and the release wheels: the long, pinned, download-and-build rules.
include Makefile.build
