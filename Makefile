# NetHack Babel - Makefile
#
# Targets:
#   build     - debug build (default)
#   release   - optimized release build
#   install   - build release and install (user-local)
#   test      - run all tests
#   check     - run cargo check (fast type-checking)
#   clippy    - run clippy lints
#   fmt       - format code with rustfmt
#   fmt-check - check formatting without modifying
#   clean     - remove build artifacts

.PHONY: build release install test check clippy fmt fmt-check clean

build:
	cargo build

release:
	cargo build --release

install: release
	./install.sh

test:
	cargo test --workspace

check:
	cargo check --workspace

clippy:
	cargo clippy --workspace -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clean:
	cargo clean
