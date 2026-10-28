# Models

Every request and response type lives in `autumn_sdk::models`, with the names the JS SDK uses (`CheckParams`, `AttachResponse`, `GetCustomerResponse`, ...). They are generated from the JS SDK's schemas, so each field carries Autumn's documentation. `cargo doc --open` is the best way to browse them.

## Field names

Fields use the API's snake_case wire names, so they match Autumn's REST documentation. Keywords are escaped with raw identifiers, for example `TrackParams::r#async`.

| JS SDK (camelCase) | Rust / wire |
|---|---|
| `customerId` | `customer_id` |
| `sendEvent` | `send_event` |
| `async` | `r#async` |

## Building requests

Each request type has a `new` constructor that takes the **required** fields and a chainable setter per **optional** field:

```rust
use autumn_sdk::models::{AttachFeatureQuantity, AttachParams};

let params = AttachParams::new("cus_123", "pro")
    .success_url("https://example.com/welcome")
    .feature_quantities(vec![AttachFeatureQuantity::new("seats").quantity(5.0)]);
```

Constructors and setters accept anything that converts into the field type (`impl Into<T>`), so `&str`, `String`, arrays and the like work directly.

All fields are public, so you can also mutate a value after building it. Many types also implement `Default`, which allows struct-update syntax:

```rust
let mut params = CheckParams::new("cus_123", "messages");
params.required_balance = Some(5.0);
```

## Optional vs. nullable request fields

Some optional request fields also accept `null`, which usually means "clear this value". These fields are `Option<Option<T>>`:

| Value | Sent as |
|---|---|
| `None` | field omitted (leave unchanged) |
| `Some(None)` | `null` (clear) |
| `Some(Some(v))` | `v` |

The setter (`.name("Jane")`) always sets `Some(Some(v))`. To clear a field, assign it directly:

```rust
let mut update = UpdateCustomerParams::new("cus_123");
update.email = Some(None); // remove the customer's email
```

Required fields that may be `null` are `Option<T>`. Their constructor argument takes the value itself. To send `null`, set the field to `None` afterwards.

## Responses

Response fields that may be missing or `null` are `Option<T>`. Required primitive fields are read leniently, like the JS SDK reads them: a `null` becomes `0`, `""` or `false`, and numbers or booleans sent as strings are accepted.

Most responses are plain structs, so you can access fields directly:

```rust
let check = autumn.check(CheckParams::new("cus_123", "messages")).await?;
if check.allowed {
    let remaining = check.balance.as_ref().map(|b| b.remaining);
}
```

The JS SDK sometimes declares one response schema per status code. `check`, for example, has separate 200 and 202 bodies. When those schemas are identical, the Rust SDK collapses them into one type: `CheckResponse` is an alias of `CheckResponseBody1`.

## Enums

String enums are Rust enums with `as_str()`, `Display` and `FromStr`:

```rust
use autumn_sdk::models::FinalizeLockAction;

let action: FinalizeLockAction = "confirm".parse().unwrap();
assert_eq!(action.as_str(), "confirm");
```

* **Request enums** are *closed*: an unknown value fails to parse.
* **Response enums** are *open*: they have an extra `Unknown(String)` variant, so new values Autumn adds server-side don't break your deserialization. Match with a wildcard arm, or call `is_unknown()`.

## Unions

Fields that accept several shapes are untagged enums, for example a value that is either a number or the string `"inf"`. Variant names describe the shape (`Number`, `String`, `StringArray`, ...), and `From` conversions are implemented where they are unambiguous:

```rust
use autumn_sdk::models::{EventsAggregateParams, EventsAggregateParamsFeatureIdUnion};

let one = EventsAggregateParams::new("messages"); // From<&str>
let many = EventsAggregateParams::new(EventsAggregateParamsFeatureIdUnion::StringArray(vec![
    "messages".into(),
    "tokens".into(),
]));
```

## Literal types

A few fields can only hold a single value, such as `Lock::enabled`, which is always `true`. Their types live in `models::literals` and serialize to exactly that value. Constructors fill them in for you.

## Approval-rule filters

Approval rules use a MongoDB-style condition language (`$and`, `$or`, `$in`, `$some`, ...). The JS SDK expands it into thousands of generated types. Here it is `models::ApprovalRuleFilter`, an alias for `serde_json::Value`:

```rust
use serde_json::json;

let allow = json!({
    "plan_id": { "$in": ["pro", "team"] },
    "items": { "$some": { "billing_method": "prepaid" } }
});
```

You can also use `json!(true)` or `json!(false)` to always or never allow.

## Serde

Every model implements `Serialize`, `Deserialize`, `Debug`, `Clone` and `PartialEq`. You can store responses, log them, or build requests from JSON:

```rust
let params: CheckParams = serde_json::from_value(json!({
    "customer_id": "cus_123",
    "feature_id": "messages"
}))?;
```
