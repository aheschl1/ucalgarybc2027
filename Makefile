.PHONY: setup dev sdk viewer-types viewer web wheels test test-api lint clean up down db deploy release-check release

# `make <target> GAME=tictactoe` builds one game everywhere: the engine with that cargo
# feature, the SDK with that game's handle, the viewer and web app with its renderer, the
# API image likewise. Unset: every game. Exported so npm and compose see the same choice.
GAME ?=
FEATURES := $(if $(GAME),--no-default-features -F $(GAME),)
export UCBC_GAME := $(GAME)

# `ENV=prod make up` substitutes .env.prod into compose instead of .env.local.
ENV ?= local
COMPOSE := docker compose --env-file .env --env-file .env.$(ENV)

PY := ucbc-sdk ucbc-py/python ucbc-api ucbc-worker alembic tests bots

# First setup: the engine extension into .venv, and the viewer.
setup: viewer
	uv sync

# After Rust changes: regenerate the SDK and rebuild the extension.
dev: sdk
	cd ucbc-py && uv run maturin develop --uv $(FEATURES)

node_modules: package-lock.json
	npm ci
	touch $@

sdk:
	cargo run -q -p ucbc-dev -- gen-sdk ucbc-sdk/ucbc/games

# TypeScript types for the viewer, from the replay schema and each game's API.
viewer-types: node_modules
	node ucbc-viewer/scripts/gen-types.mjs

# The viewer page, built into the ucbc_engine package for `ucbc view`.
viewer: node_modules
	npm run build -w @ucbc/viewer -- --outDir ../ucbc-py/python/ucbc_engine/viewer/static --emptyOutDir

# The platform frontend, built into the API package so `ucbc-api` serves it at /.
web: node_modules
	npm run build -w ucbc-web -- --outDir ../ucbc-api/api/static --emptyOutDir

# Release files into dist/: ucbc (engine, runtime, CLI) as one abi3 manylinux wheel per
# TARGETS entry, cross-compiled with zig (`rustup target add` each once), and ucbc-sdk.
# ucbc gets no sdist: a platform without a wheel should fail to find one, not try to build.
TARGETS := x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
wheels: sdk viewer
	rm -rf dist && mkdir -p dist
	for t in $(TARGETS); do \
	  uv run maturin build --release --zig --compatibility manylinux2014 --target $$t \
	    -m ucbc-py/Cargo.toml -o dist $(FEATURES) || exit 1; done
ifeq ($(GAME),)
	uv build --package ucbc-sdk -o dist
else
	tmp=$$(mktemp -d) && cp -r ucbc-sdk $$tmp/sdk \
	  && find $$tmp/sdk/ucbc/games -mindepth 1 -maxdepth 1 -type d ! -name $(GAME) -exec rm -rf {} + \
	  && uv build -o dist $$tmp/sdk && rm -rf $$tmp
endif

# Publishing: `make release` uploads dist/ to PyPI with PYPI_API_TOKEN (from the shell, else
# .env.prod) and tags the commit. A version can be uploaded once, ever. release-check refuses
# versions that disagree or a dirty tree, then installs the x86_64 wheel in clean containers
# and plays a match.
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' ucbc-py/pyproject.toml)
release-check: wheels
	@grep -q '^version = "$(VERSION)"' ucbc-sdk/pyproject.toml \
	  && grep -q '^version = "$(VERSION)"' Cargo.toml \
	  && grep -q '"ucbc-sdk==$(VERSION)"' ucbc-py/pyproject.toml \
	  || { echo "versions disagree with ucbc-py $(VERSION)"; exit 1; }
	@test -z "$$(git status --porcelain)" || { echo "commit or stash first"; exit 1; }
	uvx twine check dist/*
	for py in 3.12 3.14; do \
	  docker run --rm -v $(CURDIR)/dist:/dist:ro -v $(CURDIR)/bots:/bots:ro python:$$py-slim sh -c \
	    "pip install -q --find-links /dist ucbc==$(VERSION) && ucbc run /bots/tictactoe/random /bots/tictactoe/first_empty" \
	    || exit 1; done

release: release-check
	@UV_PUBLISH_TOKEN=$${PYPI_API_TOKEN:-$$(sed -n 's/^PYPI_API_TOKEN=//p' .env.prod)} uv publish dist/*
	git tag v$(VERSION)

# tests/api starts its own Postgres through testcontainers; needs Docker.
test: dev node_modules
	cargo test --workspace
	uv run pytest
	npm test --workspaces --if-present

test-api: dev
	uv run pytest tests/api

lint: node_modules
	cargo run -q -p ucbc-dev -- gen-sdk ucbc-sdk/ucbc/games --check
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	uv run ruff check $(PY)
	uv run ruff format --check $(PY)
	uv run mypy
	node ucbc-viewer/scripts/gen-types.mjs --check
	npm run typecheck --workspaces

clean:
	cargo clean
	rm -rf .venv dist node_modules ucbc-api/api/static

# Platform: Postgres, the API with the web app, and a worker. `make up` builds the images,
# migrates, and serves on :8000. `make db` is Postgres alone, for running the API from
# the checkout. COMPOSE_PROFILES in .env.$(ENV) decides whether `up` starts Postgres.
up:
	$(COMPOSE) up --build

down:
	$(COMPOSE) down

db:
	$(COMPOSE) up -d --wait db

# Production on another host, which builds nothing: images built here are loaded there and
# started as ENV=prod. DEPLOY_SSH is the ssh command that reaches it. That host needs
# Docker, and ~/ucbc/.env and .env.prod (from .env.prod.example) written by hand, since
# neither is in git. `make deploy SERVICES="db minio"` starts only those.
DEPLOY_SSH ?= ssh ucbc-vm
SERVICES ?=
deploy:
	docker compose build api worker
	docker save ucbc-api:latest ucbc-worker:latest | gzip | $(DEPLOY_SSH) 'gunzip | docker load'
	tar -c --exclude=__pycache__ compose.yaml bots | $(DEPLOY_SSH) 'mkdir -p ucbc && tar -x -C ucbc'
	$(DEPLOY_SSH) 'cd ucbc && ENV=prod docker compose --env-file .env --env-file .env.prod up -d --no-build $(SERVICES)'
