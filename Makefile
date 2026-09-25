.PHONY: help setup dev gen sdk viewer-types viewer web wheels test test-api lint clean up down db deploy release-check release
.DEFAULT_GOAL := help

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

help:
	@grep -hE '^[a-z-]+:.*##' $(MAKEFILE_LIST) | sed 's/:.*##\s*/\t/' | column -ts '	' | sed 's/^/  /'

setup: .env .env.local gen viewer  ## first setup: env files, generated code, viewer, .venv with the engine
	uv sync

dev: gen  ## after Rust changes: regenerate the SDK and viewer types, rebuild the extension
	cd ucbc-py && uv run maturin develop --uv $(FEATURES)

test: dev .env .env.local  ## rust, python, api (needs Docker), viewer
	cargo test --workspace
	uv run pytest
	npm test --workspaces --if-present

test-api: dev .env .env.local  ## API and worker tests alone
	uv run pytest tests/api

lint: gen  ## cargo fmt/clippy, ruff, mypy, tsc; regenerates first, so a stale checkout shows in git status
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	uv run ruff check $(PY)
	uv run ruff format --check $(PY)
	uv run mypy
	npm run typecheck --workspaces

up: .env .env.$(ENV)
	$(COMPOSE) up --build

down:  ## stop compose
	$(COMPOSE) down

clean:  ## build outputs, .venv, node_modules
	cargo clean
	rm -rf .venv dist node_modules ucbc-api/api/static

# Pieces

.env .env.local:
	cp $@.example $@

node_modules: package-lock.json
	npm ci
	touch $@

# Code generated from the Rust types: the Python SDK and the viewer's TypeScript types.
# Committed, so a fresh checkout works without cargo; regenerated before anything uses it.
gen: sdk viewer-types

sdk:
	cargo run -q -p ucbc-dev -- gen-sdk ucbc-sdk/ucbc/games

viewer-types: node_modules
	node ucbc-viewer/scripts/gen-types.mjs

# The viewer page, built into the ucbc_engine package for `ucbc view`.
viewer: viewer-types
	npm run build -w @ucbc/viewer -- --outDir ../ucbc-py/python/ucbc_engine/viewer/static --emptyOutDir

# The platform frontend, built into the API package so `ucbc-api` serves it at /.
web: viewer-types
	npm run build -w ucbc-web -- --outDir ../ucbc-api/api/static --emptyOutDir

# Postgres alone, for running the API from the checkout.
db: .env .env.$(ENV)
	$(COMPOSE) up -d --wait db

# Release

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

# Publishing: `ENV=prod make release` uploads dist/ to PyPI and tags the commit; any other
# ENV is refused. PYPI_API_TOKEN comes from the environment, else .env.$(ENV) over .env,
# like every other setting. A version can be uploaded once, ever. release-check refuses
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

release:  ## ENV=prod make release: wheels to PyPI, tag the commit
	@test "$(ENV)" = prod || { echo "publishing is a prod action: ENV=prod make release"; exit 1; }
	$(MAKE) release-check
	@UV_PUBLISH_TOKEN=$${PYPI_API_TOKEN:-$$(cat .env .env.$(ENV) | sed -n 's/^PYPI_API_TOKEN=//p' | tail -1)} \
	  uv publish dist/*
	git tag v$(VERSION)

# Production on another host, which builds nothing: images built here are loaded there and
# started as ENV=prod. DEPLOY_SSH is the ssh command that reaches it. That host needs
# Docker, and ~/ucbc/.env and .env.prod (from .env.prod.example) written by hand, since
# neither is in git. `make deploy SERVICES="db minio"` starts only those.
DEPLOY_SSH ?= ssh ucbc-vm
SERVICES ?=
deploy:  ## build the images here, load and start them on DEPLOY_SSH as prod
	docker compose build api worker
	docker save ucbc-api:latest ucbc-worker:latest | gzip | $(DEPLOY_SSH) 'gunzip | docker load'
	tar -c --exclude=__pycache__ compose.yaml bots | $(DEPLOY_SSH) 'mkdir -p ucbc && tar -x -C ucbc'
	$(DEPLOY_SSH) 'cd ucbc && ENV=prod docker compose --env-file .env --env-file .env.prod up -d --no-build $(SERVICES)'
