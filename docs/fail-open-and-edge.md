# Fail-open and edge routing

## Fail-open

Billing infrastructure should never take your product down. Like the JS SDK, `check` and `track` **fail open** by default: if the call can't complete, the SDK logs an error and returns a permissive response instead of an `Err`.

A call fails open when any of these happens:

* Autumn can't be reached (DNS, connection refused or reset, TLS failure).
* The call times out. Without a configured timeout, `check` and `track` time out after **5 seconds**.
* Autumn answers with a **5XX** status. Fail-open calls are answered immediately and not retried.

The response you get back:

| Call | Fail-open response |
|---|---|
| `check` | `allowed: true`, `customer_id: ""`, `balance: None`, `flag: None` |
| `track` | `customer_id: ""`, `value: 0.0`, `balance: None` |

4XX errors (bad key, unknown customer, invalid request) are **not** failed open. They are real bugs and come back as `Error::Api`.

Every fail-open is logged at `error` level under the `autumn_sdk` log target:

```text
[Autumn] Request failed — failing open. Learn more: https://docs.useautumn.com/documentation/fail-open
  Operation: check | Error: Request timed out
```

If you'd rather deny access when Autumn is unavailable, turn fail-open off:

```rust
let autumn = Autumn::builder().fail_open(false).build()?;
// or AUTUMN_FAIL_OPEN=false
```

Separately, Autumn itself may answer `check` with **202 Accepted** when a downstream provider is degraded. Access is then allowed server-side. The SDK treats the 202 body like a 200.

> **Difference from autumn-js:** with fail-open on, the JS SDK turns network failures on *every* operation into a synthetic `555` API error. This crate fails open only for `check` and `track`. Other operations return `Error::Connection` or `Error::Timeout`, which are easier to handle in Rust.

## Edge routing

Autumn can run an edge in front of the API to answer latency-sensitive calls closer to your servers. Only `check` and `balances().finalize` are served by the edge. Every other call always goes to the API.

```rust
let autumn = Autumn::builder()
    .edge_url("https://edge.example.com")
    .build()?;

// Opt in per call...
autumn.edge().check(CheckParams::new("cus_123", "messages")).await?;

// ...or for every call made through the client.
let autumn = Autumn::builder().edge_url("https://edge.example.com").use_edge(true).build()?;
```

What happens on an edge call:

1. The request goes to the edge with a 1-second budget.
2. The edge's answer is used unless the edge was unreachable, timed out, or returned a 5XX *without* the `x-atom-forwarded` header. A forwarded 5XX came from the API itself and is used as-is.
3. Otherwise the same request is sent to the API.

Calls that change state (`finalize`, and `check` with `send_event: true` or a `lock`) get an automatically generated `Idempotency-Key`. That way, a request that reached the edge before falling back to the API is never counted twice. To set your own key, use `RequestOptions::idempotency_key`.

Per-call `server_url` overrides bypass the edge.
