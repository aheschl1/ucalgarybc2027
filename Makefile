.PHONY: help setup dev gen sdk viewer-types viewer web runtime guest wheels test test-api lint clean up down db release-check release
.DEFAULT_GOAL := help

# Builds include ucbc-games' default games. The tests build every game: they play
# tic-tac-toe, the bot runtime's fixture.
test test-api: FEATURES = -F ucbc-games/all

# `ENV=prod make up` substitutes .env.prod into compose instead of .env.local.
ENV ?= local
COMPOSE := docker compose --env-file .env --env-file .env.$(ENV)

PY := ucbc-cli/python ucbc-api ucbc-worker alembic tests bots

help:
	@grep -hE '^[a-z-]+:.*##' $(MAKEFILE_LIST) | sed 's/:.*##\s*/\t/' | column -ts '	' | sed 's/^/  /'

setup: .env .env.local dev  ## once: env files, then everything `make dev` does

# uv builds the engine, here and only here: a `uv run` afterwards leaves the build alone.
dev: gen viewer runtime  ## after any change: generated code, the viewer page, the bot runtime, dependencies and the engine in .venv
	MATURIN_PEP517_ARGS="$(FEATURES)" uv sync --all-packages --reinstall-package ucbc

test: dev .env .env.local  ## rust, python, api (needs Docker), viewer
	cargo test --workspace
	uv run pytest
	npm test --workspaces --if-present

test-api: dev .env .env.local
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

clean:  ## build outputs, .venv, node_modules (not the downloads in .cache)
	cargo clean
	rm -rf .venv dist node_modules ucbc-api/api/static $(RUNTIME)

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
	cargo run -q -p ucbc-dev -- gen-sdk ucbc-cli/python/ucbc/games

viewer-types: node_modules
	node ucbc-viewer/scripts/gen-types.mjs

# The viewer page, built into the ucbc package for `ucbc view`.
viewer: viewer-types
	npm run build -w @ucbc/viewer -- --outDir ../ucbc-cli/python/ucbc/viewer/static --emptyOutDir

# The platform frontend, built into the API package so `ucbc-api` serves it at /.
web: viewer-types
	npm run build -w ucbc-web -- --outDir ../ucbc-api/api/static --emptyOutDir

# The bot runtime (ucbc-wasm), into the ucbc package: the guest (CPython for WASI linked
# with ucbc-wasm/guest) snapshotted once Python has started and precompiled for this
# machine, and the stdlib as one zip. Guest and zip are downloaded from a release that
# .github/workflows/guest.yml built; `make guest` builds them here instead, which needs
# the wasm32-wasip1 target and three more downloads. Downloads land in .cache, checked
# against their sha256, and survive `make clean`.
RUNTIME := ucbc-cli/python/ucbc/runtime
CACHE := .cache
BUILD := $(CACHE)/build
SNAPSHOT := cargo run -q --release -p ucbc-dev -- snapshot
GUEST_RELEASE := https://github.com/aheschl1/ucalgarybc2027/releases/download/guest-1
GUEST_SHA256 :=
STDLIB_SHA256 :=
SHA256 := $(shell command -v sha256sum || echo shasum -a 256)

runtime: $(RUNTIME)/bot.cwasm

$(RUNTIME)/bot.cwasm: $(CACHE)/guest.wasm $(RUNTIME)/lib/python314.zip $(shell find ucbc-cli/python/ucbc -name '*.py' -not -path '*/runtime/*')
	$(SNAPSHOT) $< ucbc-cli/python/ucbc $@

$(RUNTIME)/lib/python314.zip: $(CACHE)/stdlib.zip
	mkdir -p $(@D) && cp $< $@

# Until the first release exists the checksums are empty and the guest is built here.
ifneq ($(GUEST_SHA256),)
$(CACHE)/guest.wasm:
	$(call fetch,$(GUEST_RELEASE)/guest.wasm,$(GUEST_SHA256))
$(CACHE)/stdlib.zip:
	$(call fetch,$(GUEST_RELEASE)/stdlib.zip,$(STDLIB_SHA256))
else
$(CACHE)/guest.wasm: $(BUILD)/guest.wasm
	cp $< $@
$(CACHE)/stdlib.zip: $(BUILD)/stdlib.zip
	cp $< $@
endif

# Building the guest: CPython's WASI build from a pinned release, the guest crate linked
# against it, and the stdlib zipped without what a bot cannot use.
CPYTHON_WASI := https://github.com/brettcannon/cpython-wasi-build/releases/download/v3.14.7
WASI_SDK := https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-24
PRUNE := asyncio concurrent ctypes curses dbm email ensurepip html http idlelib lib-dynload \
  multiprocessing pydoc_data _pyrepl site-packages sqlite3 test tkinter turtledemo unittest \
  urllib venv wsgiref xml xmlrpc doctest.py imaplib.py mailbox.py pdb.py pydoc.py smtplib.py \
  ssl.py turtle.py

guest: $(BUILD)/guest.wasm $(BUILD)/stdlib.zip  ## build the guest interpreter and stdlib zip locally (needs the wasm32-wasip1 target)
	cp $^ $(CACHE)

$(BUILD)/guest.wasm: $(shell find ucbc-wasm/guest -type f -not -path '*/target/*') $(BUILD)/python-build $(BUILD)/wasi-sysroot-24.0
	cd ucbc-wasm/guest && cargo build -q --release
	cp target/wasm32-wasip1/release/ucbc_guest.wasm $@

# Stored, not deflated: the interpreter has no zlib. `-I` keeps the host Python from
# importing the 3.14 stdlib it is standing in.
$(BUILD)/stdlib.zip: $(BUILD)/python.zip
	rm -rf $(BUILD)/python && python3 -m zipfile -e $< $(BUILD)/python
	cd $(BUILD)/python/lib/python3.14 && rm -rf $(PRUNE) && find . -name __pycache__ -prune -exec rm -rf {} + \
	  && python3 -I -c 'import os, sys, zipfile; z = zipfile.ZipFile(sys.argv[1], "w", zipfile.ZIP_STORED); [z.write(os.path.join(d, f)) for d, _, fs in sorted(os.walk(".")) for f in sorted(fs)]' $(CURDIR)/$@

$(BUILD)/python-build: $(BUILD)/python-build.zip
	rm -rf $@ && python3 -m zipfile -e $< $@

$(BUILD)/wasi-sysroot-24.0: $(BUILD)/wasi-sysroot.tar.gz
	tar -xzf $< -C $(BUILD) wasi-sysroot-24.0/lib/wasm32-wasip1 && touch $@

$(BUILD)/python.zip:
	$(call fetch,$(CPYTHON_WASI)/python-3.14.7-wasi_sdk-24.zip,2e064d3fb8172471d39d741348efa722349c40b96301f69968dff714999c584b)
$(BUILD)/python-build.zip:
	$(call fetch,$(CPYTHON_WASI)/_build-python-3.14.7-wasi_sdk-24.zip,a2a9fca77dc47bb9d2b0dd7d04538f8aa188e5f0a87b362fd569c0d024039100)
$(BUILD)/wasi-sysroot.tar.gz:
	$(call fetch,$(WASI_SDK)/wasi-sysroot-24.0.tar.gz,35172f7d2799485b15a46b1d87f50a585d915ec662080f005d99153a50888f08)

define fetch
mkdir -p $(@D) && curl -fsSL -o $@.part $(1)
echo "$(2)  $@.part" | $(SHA256) -c - >/dev/null && mv $@.part $@
endef

# Postgres alone, for running the API from the checkout.
db: .env .env.$(ENV)
	$(COMPOSE) up -d --wait db

# Release

# Release files into dist/: ucbc as one abi3 manylinux wheel per TARGETS entry,
# cross-compiled with zig (`rustup target add` each once). No sdist: a platform without a
# wheel should fail to find one, not try to build.
# The runtime is precompiled per target, so the snapshot is redone for each wheel and
# removed after, for the next `make dev` to rebuild for this machine.
TARGETS := x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
wheels: sdk viewer $(CACHE)/guest.wasm $(RUNTIME)/lib/python314.zip
	rm -rf dist && mkdir -p dist
	for t in $(TARGETS); do \
	  $(SNAPSHOT) --target $$t $(CACHE)/guest.wasm ucbc-cli/python/ucbc $(RUNTIME)/bot.cwasm && \
	  uv run maturin build --release --zig --compatibility manylinux2014 --target $$t \
	    -m ucbc-cli/Cargo.toml -o dist || exit 1; done
	rm $(RUNTIME)/bot.cwasm

# Publishing: `ENV=prod make release` uploads dist/ to PyPI and tags the commit; any other
# ENV is refused. PYPI_API_TOKEN comes from the environment, else .env.$(ENV) over .env,
# like every other setting. A version can be uploaded once, ever. release-check refuses
# versions that disagree or a dirty tree, then installs the x86_64 wheel in clean containers
# and plays a match.
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' ucbc-cli/pyproject.toml)
release-check: wheels
	@grep -q '^version = "$(VERSION)"' Cargo.toml \
	  || { echo "versions disagree with ucbc-cli $(VERSION)"; exit 1; }
	@test -z "$$(git status --porcelain)" || { echo "commit or stash first"; exit 1; }
	uvx twine check dist/*
	for py in 3.12 3.14; do \
	  docker run --rm -v $(CURDIR)/dist:/dist:ro -v $(CURDIR)/bots:/bots:ro python:$$py-slim sh -c \
	    "pip install -q --find-links /dist ucbc==$(VERSION) && ucbc run /bots/ucbc2027/noop /bots/ucbc2027/noop" \
	    || exit 1; done

release:  ## ENV=prod make release: wheels to PyPI, tag the commit
	@test "$(ENV)" = prod || { echo "publishing is a prod action: ENV=prod make release"; exit 1; }
	$(MAKE) release-check
	@UV_PUBLISH_TOKEN=$${PYPI_API_TOKEN:-$$(cat .env .env.$(ENV) | sed -n 's/^PYPI_API_TOKEN=//p' | tail -1)} \
	  uv publish dist/*
	git tag v$(VERSION)
