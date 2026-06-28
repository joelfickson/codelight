.PHONY: build run fmt fmt-check lint test check clean spike

build:
	cargo build

run:
	cargo run -p vc-cli

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

check: fmt-check lint test

clean:
	cargo clean

spike:
	cargo run -p vc-gateway --example spike
