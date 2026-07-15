.PHONY: lint build install fmt clippy

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
