.PHONY: check test fmt clippy draft-truth draft-truth-write draft-support-matrix draft-compatibility draft-benchmark

check:
	cargo check --workspace

test:
	cargo test --workspace

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

draft-truth:
	cargo run -q -p xtask -- draft-v1 truth check

draft-truth-write:
	cargo run -q -p xtask -- draft-v1 truth write

draft-support-matrix:
	cargo run -q -p xtask -- draft-v1 support-matrix check

draft-compatibility:
	cargo run -q -p xtask -- draft-v1 compatibility check

draft-benchmark:
	cargo run -q -p xtask -- draft-v1 benchmark check
