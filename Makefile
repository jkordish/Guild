.PHONY: check test fmt fmt-check clippy draft-truth draft-truth-write draft-support-matrix draft-compatibility draft-benchmark patent-packet project-positioning axiom-plan verify

check:
	cargo check --workspace

test:
	cargo test --workspace

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

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

patent-packet:
	cargo run -q -p xtask -- patent-packet check

project-positioning:
	cargo run -q -p xtask -- project-positioning check

axiom-plan:
	cargo run -q -p xtask -- axiom-plan validate-examples

verify: fmt-check test clippy draft-truth project-positioning
