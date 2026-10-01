.PHONY: help setup build gen sdk viewer-types viewer web runtime guest cpython wheels test test-api lint clean up down db release-check tag
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

setup: .env .env.local build  ## once: env files, then everything `make build` does

# uv builds the engine, here and only here: a `uv run` afterwards leaves the build alone.
build: gen viewer runtime  ## after any change: generated code, the viewer page, the bot runtime, dependencies and the engine in .venv
	MATURIN_PEP517_ARGS="$(FEATURES)" uv sync --all-packages --reinstall-package ucbc

test: build .env .env.local  ## rust, python, api (needs Docker), viewer
	cargo test --workspace
	uv run pytest
	npm test --workspaces --if-present

test-api: build .env .env.local
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
GUEST_RELEASE := https://github.com/aheschl1/ucalgarybc2027/releases/download/guest-4
GUEST_SHA256 := 566e4a58ccd1123c0704a5c84d5d32f87d8fab8f09eae81cdac0a05f1230c8b4
STDLIB_SHA256 := 8c36afdf444548ce9a10bbe07915b4503d274ccd3f99d5b559d9ce1e441b7cee
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

# Building the guest: CPython for WASI from a pinned release of ours (`make cpython`,
# below), the guest crate linked against it, and the stdlib zipped without what a bot
# cannot use.
CPYTHON_RELEASE := https://github.com/aheschl1/ucalgarybc2027/releases/download/cpython-2
CPYTHON_BUILD_SHA256 := 45364f2130c9b5e1c66ce382952faaa688f807792ce4119741eca2f11c55fb0b
CPYTHON_LIB_SHA256 := 0c95009010f7ab4290ff47d077aa7860f796b143f2eede297220679f1c5abfa5
WASI_SDK := https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-24
PRUNE := asyncio concurrent ctypes curses dbm email ensurepip html http idlelib lib-dynload \
  multiprocessing pydoc_data _pyrepl site-packages sqlite3 test tkinter turtledemo unittest \
  urllib venv wsgiref xml xmlrpc doctest.py imaplib.py mailbox.py pdb.py pydoc.py smtplib.py \
  ssl.py turtle.py

guest: $(BUILD)/guest.wasm $(BUILD)/stdlib.zip  ## build the guest interpreter and stdlib zip locally (needs the wasm32-wasip1 target)
	cp $^ $(CACHE)

BINARYEN := https://github.com/WebAssembly/binaryen/releases/download/version_123
WASM_OPT := $(BUILD)/binaryen-version_123/bin/wasm-opt

$(BUILD)/guest.wasm: $(shell find ucbc-wasm/guest -type f -not -path '*/target/*') $(BUILD)/python-build $(BUILD)/wasi-sysroot-24.0 $(WASM_OPT)
	cd ucbc-wasm/guest && cargo build -q --release
	$(WASM_OPT) -O3 target/wasm32-wasip1/release/ucbc_guest.wasm -o $@

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

# Until the first release exists the checksums are empty and CPython is built here.
ifneq ($(CPYTHON_BUILD_SHA256),)
$(BUILD)/python-build.zip:
	$(call fetch,$(CPYTHON_RELEASE)/python-build.zip,$(CPYTHON_BUILD_SHA256))
$(BUILD)/python.zip:
	$(call fetch,$(CPYTHON_RELEASE)/python.zip,$(CPYTHON_LIB_SHA256))
else
$(BUILD)/python-build.zip $(BUILD)/python.zip: cpython
endif
$(BUILD)/wasi-sysroot.tar.gz:
	$(call fetch,$(WASI_SDK)/wasi-sysroot-24.0.tar.gz,35172f7d2799485b15a46b1d87f50a585d915ec662080f005d99153a50888f08)

# CPython for WASI (wasi-sdk 24, CPython's own config.site) 
# plus pymalloc, which configure leaves out on WASI, LTO, and
# the wasm features wasmtime has: together 7-10% less fuel per step. The archives hold
# bitcode; the guest's link runs the LTO. .github/workflows/cpython.yml runs this and
# publishes both zips, Linux x86_64 only: the wasi-sdk (100 MB), Python 3.14 through uv
# as the build Python, about five minutes. Entries are dated so a rebuild of the same
# source gives the same checksums.
CPYTHON_VERSION := 3.14.7
CPYTHON_SRC := $(BUILD)/Python-$(CPYTHON_VERSION)
CPYTHON_OUT := $(CPYTHON_SRC)/cross-build/wasm32-wasip1
CPYTHON_SDK := $(CURDIR)/$(BUILD)/wasi-sdk-24.0-x86_64-linux
CPYTHON_SYSROOT := $(CPYTHON_SDK)/share/wasi-sysroot
CPYTHON_CFLAGS := -mnontrapping-fptoint -msign-ext -mmutable-globals
CPYTHON_LIBS := libpython3.14.a Modules/_decimal/libmpdec/libmpdec.a Modules/expat/libexpat.a \
  $(addprefix Modules/_hacl/libHacl_,Hash_MD5.a Hash_SHA1.a Hash_SHA2.a Hash_SHA3.a Hash_BLAKE2.a HMAC.a)
CPYTHON_ENV := CC=$(CPYTHON_SDK)/bin/clang CPP=$(CPYTHON_SDK)/bin/clang-cpp AR=$(CPYTHON_SDK)/bin/llvm-ar \
  RANLIB=$(CPYTHON_SDK)/bin/ranlib PKG_CONFIG_PATH= PKG_CONFIG_SYSROOT_DIR=$(CPYTHON_SYSROOT) \
  PKG_CONFIG_LIBDIR=$(CPYTHON_SYSROOT)/lib/pkgconfig:$(CPYTHON_SYSROOT)/share/pkgconfig \
  WASI_SDK_PATH=$(CPYTHON_SDK) WASI_SYSROOT=$(CPYTHON_SYSROOT) HOSTRUNNER=true \
  PATH=$(CPYTHON_SDK)/bin:$$PATH

cpython: $(CPYTHON_SRC) $(CPYTHON_SDK)  ## build CPython for WASI into .cache/build/python-build.zip and python.zip (Linux x86_64; needs uv)
	mkdir -p $(CPYTHON_OUT) && cd $(CPYTHON_OUT) && $(CPYTHON_ENV) \
	  CONFIG_SITE=../../Tools/wasm/wasi/config.site-wasm32-wasi ../../configure -q \
	  --host=wasm32-wasip1 --build=x86_64-pc-linux-gnu --with-build-python=$$(uv python find $(CPYTHON_VERSION)) \
	  --with-pymalloc --with-lto=full CFLAGS="$(CPYTHON_CFLAGS)"
	$(MAKE) -s -C $(CPYTHON_OUT) -j$(shell nproc) HOSTRUNNER=true pybuilddir.txt $(CPYTHON_LIBS)
	rm -rf $(BUILD)/python-lib && mkdir -p $(BUILD)/python-lib/lib/python3.14 \
	  && cp -r $(CPYTHON_SRC)/Lib/. $(CPYTHON_OUT)/build/lib.wasi-wasm32-3.14/_sysconfigdata__wasi_wasm32-wasi.py $(BUILD)/python-lib/lib/python3.14 \
	  && find $(BUILD)/python-lib -name __pycache__ -prune -exec rm -rf {} +
	find $(BUILD)/python-lib $(addprefix $(CPYTHON_OUT)/,$(CPYTHON_LIBS) pyconfig.h config.log) -exec touch -d 2020-01-01T00:00:00Z {} +
	rm -f $(BUILD)/python-build.zip $(BUILD)/python.zip
	cd $(CPYTHON_OUT) && TZ=UTC zip -q -X $(CURDIR)/$(BUILD)/python-build.zip $(CPYTHON_LIBS) pyconfig.h config.log
	cd $(BUILD)/python-lib && TZ=UTC zip -q -X -r $(CURDIR)/$(BUILD)/python.zip lib

$(CPYTHON_SRC): $(BUILD)/Python-$(CPYTHON_VERSION).tar.xz
	rm -rf $@ && tar -xJf $< -C $(BUILD) && echo "# Edit this file for local setup changes" > $@/Modules/Setup.local
$(CPYTHON_SDK): $(BUILD)/wasi-sdk.tar.gz
	tar -xzf $< -C $(BUILD) && touch $@
$(BUILD)/Python-$(CPYTHON_VERSION).tar.xz:
	$(call fetch,https://www.python.org/ftp/python/$(CPYTHON_VERSION)/Python-$(CPYTHON_VERSION).tar.xz,3b48dac8fb59f62eaa67ac83c1eb12bda1b7a08406dd286e252c11a66be27f81)
$(BUILD)/wasi-sdk.tar.gz:
	$(call fetch,$(WASI_SDK)/wasi-sdk-24.0-x86_64-linux.tar.gz,c6c38aab56e5de88adf6c1ebc9c3ae8da72f88ec2b656fb024eda8d4167a0bc5)

$(WASM_OPT): $(BUILD)/binaryen.tar.gz
	tar -xzf $< -C $(BUILD) binaryen-version_123/bin/wasm-opt && touch $@
$(BUILD)/binaryen.tar.gz:
	$(call fetch,$(BINARYEN)/binaryen-version_123-x86_64-linux.tar.gz,e959f2170af4c20c552e9de3a0253704d6a9d2766e8fdb88e4d6ac4bae9388fe)

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
# removed after, for the next `make build` to rebuild for this machine.
TARGETS := x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
wheels: sdk viewer $(CACHE)/guest.wasm $(RUNTIME)/lib/python314.zip
	rm -rf dist && mkdir -p dist
	for t in $(TARGETS); do \
	  $(SNAPSHOT) --target $$t $(CACHE)/guest.wasm ucbc-cli/python/ucbc $(RUNTIME)/bot.cwasm && \
	  uv run maturin build --profile dist --zig --compatibility manylinux2014 --target $$t \
	    -m ucbc-cli/Cargo.toml -o dist || exit 1; done
	rm $(RUNTIME)/bot.cwasm

# Publishing is CI's, keyed by tag. `make tag` pushes v$(VERSION): .github/workflows/release.yml
# runs release-check, uploads dist/ to PyPI (secret PYPI_API_TOKEN) and attaches the wheels
# to a GitHub release at the tag. `make tag KIND=guest` or `KIND=cpython` pushes the next
# guest-N or cpython-N instead, which rebuilds that artifact for this file to pin. A version
# can be uploaded once, ever. release-check refuses versions that disagree or a dirty tree,
# then installs the x86_64 wheel in clean containers and plays a match.
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

tag:  ## tag HEAD and push it: v$(VERSION) to publish; KIND=guest or KIND=cpython to rebuild that artifact
	@test -z "$(KIND)" -o "$(KIND)" = guest -o "$(KIND)" = cpython || { echo "KIND is guest or cpython"; exit 1; }
	@test -z "$$(git status --porcelain)" || { echo "commit or stash first"; exit 1; }
	@git fetch -q --tags
	@tag=$(if $(KIND),$(KIND)-$$(($$(git tag -l '$(KIND)-*' | sed 's/.*-//' | sort -n | tail -1) + 1)),v$(VERSION)); \
	  git tag $$tag && git push -q origin $$tag && echo "pushed $$tag"
