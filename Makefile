.PHONY: lint build install fmt clippy test sync-packaging check-packaging release

CARGO ?= cargo

lint: fmt clippy

fmt:
	$(CARGO) fmt --all -- --check

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

build:
	$(CARGO) build --workspace

test:
	$(CARGO) test --workspace

install:
	$(CARGO) install --path scout --locked

sync-packaging:
	$(CARGO) run -p release --bin sync-packaging

check-packaging:
	$(CARGO) run -p release --bin sync-packaging -- --check

release:
	$(CARGO) run -p release
