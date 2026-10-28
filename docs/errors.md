# Errors

Every call returns `autumn_sdk::Result<T>`, which is `Result<T, autumn_sdk::Error>`.

```rust
pub enum Error {
    Api(Box<ApiError>),                                  // 4XX/5XX (or unexpected status) from Autumn
    ResponseValidation(Box<ResponseValidationError>),    // 2XX, but the body didn't match the model
    Timeout,                                             // the call exceeded its timeout
    Connection(reqwest::Error),                          // DNS, TLS, connection reset, ...
    Serialization(serde_json::Error),                    // the request couldn't be encoded
    InvalidRequest(String),                              // e.g. an invalid server URL
}
```

The enum is `#[non_exhaustive]`, so always include a wildcard arm.

## API errors

`ApiError` carries everything Autumn returned:

| Field | |
|---|---|
| `status` | HTTP status code |
| `code` | Autumn's machine-readable error code, e.g. `customer_not_found` |
| `message` | Human-readable message |
| `body` | Raw response body (`.json::<T>()` parses it) |
| `headers`, `content_type` | Response headers |

```rust
use autumn_sdk::Error;

match autumn.customers().get(GetCustomerParams::new("cus_123")).await {
    Ok(customer) => { /* ... */ }
    Err(Error::Api(e)) if e.code.as_deref() == Some("customer_not_found") => { /* create it */ }
    Err(Error::Api(e)) if e.status == 429 => { /* back off */ }
    Err(e) => return Err(e.into()),
}
```

Shortcuts: `err.status()`, `err.code()` and `err.as_api_error()` work on any `Error`.

`Display` for API errors follows the JS SDK's `AutumnDefaultError` format: `API error occurred: Status 404. Body: {...}`. Bodies over 10,000 characters are truncated.

## Validation errors

`ResponseValidation` means Autumn answered with a success status, but the body couldn't be parsed into the expected type. This usually means the API returned a shape newer than this SDK knows. The raw `body` is kept, so you can still read it:

```rust
if let Error::ResponseValidation(e) = &err {
    let raw: serde_json::Value = serde_json::from_str(&e.body)?;
}
```

If you hit one, please open an issue and include the operation. Regenerating the models from the latest `autumn-js` usually fixes it (see [CONTRIBUTING](../CONTRIBUTING.md)).

## Errors that never happen for check and track

When fail-open is on (the default), `check` and `track` never return `Timeout`, `Connection` or 5XX `Api` errors. They return a permissive response instead. See [fail-open](fail-open-and-edge.md).
