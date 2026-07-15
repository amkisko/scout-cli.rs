.PHONY: lint build install fmt clippy sync-packaging check-packaging

CARGO ?= cargo

lint: fmt clippy

fmt:
	$(CARGO) fmt --all -- --check

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

build:
	$(CARGO) build --workspace

install:
	$(CARGO) install --path scout --locked

sync-packaging:
	$(CARGO) run -p release --bin sync-packaging

check-packaging:
	$(CARGO) run -p release --bin sync-packaging -- --check
