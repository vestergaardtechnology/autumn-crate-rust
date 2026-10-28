# Contributing

## Layout

| Path | |
|---|---|
| `src/client.rs` | Hand-written runtime: client, builder, request execution, retries, fail-open, edge routing, logging |
| `src/error.rs` | Error types |
| `src/util.rs` | Serde helpers and the `open_enum!` / `closed_enum!` / `literal!` macros used by generated code |
| `src/models/`, `src/resources/`, `src/generated_meta.rs` | **Generated.** Don't edit by hand |
| `tests/generated_fixtures.rs` | **Generated.** One round-trip test per operation |
| `tests/client.rs` | Runtime behaviour against an in-process mock HTTP server |
| `codegen/` | The generator (Node 18+) |
| `docs/` | Guides |

## Updating to a new autumn-js release

```bash
./codegen/regenerate.sh            # latest autumn-js
./codegen/regenerate.sh 1.5.28     # or a specific version
cargo test
```

Then update the version references in `README.md`, `docs/migrating-from-js.md` and `CHANGELOG.md`.

The generator works in two steps:

1. **`extract.mjs`** never executes code from the package. It statically parses the npm package's `dist/sdk/index.js` with acorn and turns every zod schema (`X$inboundSchema` / `X$outboundSchema`) into an intermediate representation. It also reads the operations: path, operation ID, request and response schemas, and the JSDoc of every SDK method. Field and type documentation comes from `dist/sdk/index.d.ts`, parsed with the TypeScript compiler API.
2. **`generate.mjs`** turns that representation into Rust:
   * objects become structs, with serde renames to the wire names;
   * enums become `closed_enum!` (requests) or `open_enum!` (responses);
   * unions become `#[serde(untagged)]` enums, ordered so the most specific variant is tried first;
   * unions of identical shapes are collapsed into their first variant;
   * every request type gets `new` and chainable setters;
   * each operation gets one method, with a compiled doc example;
   * one fixture test per operation is written.

Regeneration is deterministic. Running it twice must produce no diff.

## Checks

These are the checks CI runs:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

`cargo test` includes about 100 doc examples. Each one compiles separately against a large crate, so the first run takes a few minutes.

## Changing runtime behaviour

The runtime follows the JS SDK's hooks (`src/hooks/*` in the `autumn-js` source): fail-open, edge routing, idempotency keys, and the default timeout for `check` and `track`. If you change one, add or update a test in `tests/client.rs` and the relevant page in `docs/`.
