#!/usr/bin/env bash
# Regenerates src/models and src/resources from a published autumn-js release.
#
#   ./codegen/regenerate.sh            # latest autumn-js
#   ./codegen/regenerate.sh 1.5.28     # a specific version
#
# Nothing from the downloaded package is executed: extract.mjs only parses
# its JS bundle and type declarations.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
version="${1:-latest}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

(cd "$here" && npm install --silent --no-audit --no-fund)
(cd "$work" && npm pack --silent "autumn-js@$version" >/dev/null && tar xzf autumn-js-*.tgz)

node "$here/extract.mjs" "$work/package" "$work/ir.json"
node "$here/generate.mjs" "$work/ir.json" "$root"
cargo fmt --manifest-path "$root/Cargo.toml" 2>/dev/null || true
