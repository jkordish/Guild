#!/usr/bin/env bash
set -euo pipefail

registry_root="${1:-target/dev-local-registry/codex-local}"

cargo run -q -p guild-mcp --bin guild -- codex \
  scenario \
  --registry-root "$registry_root" \
  --scenario policy-denial-debug \
  --json
