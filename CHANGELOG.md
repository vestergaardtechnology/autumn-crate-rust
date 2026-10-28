# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-10-28

Initial release. Generated from `autumn-js@1.5.28`, Autumn API version `2.6.0`.

### Added

- `Autumn` client with a builder, environment configuration (`AUTUMN_SECRET_KEY`, `AUTUMN_X_API_VERSION`, `AUTUMN_FAIL_OPEN`, `AUTUMN_DEBUG`) and per-call `RequestOptions`.
- All 101 operations of the JS server SDK: `check`, `track`, `track_tokens` and `batch_track`, plus the `customers`, `billing`, `balances`, `plans`, `features`, `entities`, `events`, `invoices`, `accounts`, `licenses`, `logs`, `referrals`, `rewards`, `platform`, `keys`, `sandboxes`, `webhooks` and `approval_rules` resources.
- About 4,400 typed, documented models with request builders, open enums for responses and tri-state nullable request fields.
- Fail-open for `check` and `track`, a 5s default timeout for them, edge-first routing with API fallback and idempotency keys, and opt-in exponential-backoff retries that honour `Retry-After`.
- `codegen/` for regenerating the models from new `autumn-js` releases.
