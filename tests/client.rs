mod common;

use std::time::Duration;

use autumn_sdk::models::{
    CheckParams, FinalizeBalanceParams, FinalizeLockAction, GetCustomerParams, Lock, TrackParams,
    UpdateCustomerParams,
};
use autumn_sdk::{Autumn, BackoffStrategy, Error, RequestOptions, RetryConfig};
use common::{MockServer, Reply};
use serde_json::json;

fn client(server: &MockServer) -> Autumn {
    Autumn::builder()
        .secret_key("am_sk_test_123")
        .server_url(&server.url)
        .build()
        .unwrap()
}

fn check_ok() -> serde_json::Value {
    json!({
        "allowed": true,
        "customer_id": "cus_123",
        "balance": null,
        "flag": null,
        "required_balance": 3
    })
}

#[tokio::test]
async fn check_sends_snake_case_body_and_headers() {
    let server = MockServer::start(vec![Reply::json(200, check_ok())]).await;
    let autumn = client(&server);

    let res = autumn
        .check(
            CheckParams::new("cus_123", "messages")
                .required_balance(3.0)
                .send_event(true)
                .properties(
                    [("source".to_string(), json!("test"))]
                        .into_iter()
                        .collect::<std::collections::HashMap<_, _>>(),
                ),
        )
        .await
        .unwrap();
    assert!(res.allowed);
    assert_eq!(res.customer_id, "cus_123");
    assert_eq!(res.required_balance, Some(3.0));

    let reqs = server.requests();
    assert_eq!(reqs.len(), 1);
    let req = &reqs[0];
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/balances.check");
    assert_eq!(req.header("authorization"), Some("Bearer am_sk_test_123"));
    assert_eq!(
        req.header("x-api-version"),
        Some(autumn_sdk::OPENAPI_DOC_VERSION)
    );
    assert_eq!(req.header("content-type"), Some("application/json"));
    assert!(req
        .header("user-agent")
        .unwrap()
        .starts_with("autumn-sdk-rust/"));
    // No edge routing, so no idempotency key.
    assert_eq!(req.header("idempotency-key"), None);
    assert_eq!(
        req.json(),
        json!({
            "customer_id": "cus_123",
            "feature_id": "messages",
            "required_balance": 3.0,
            "send_event": true,
            "properties": { "source": "test" }
        })
    );
}

#[tokio::test]
async fn nullable_fields_distinguish_absent_and_null() {
    let mut params = UpdateCustomerParams::new("cus_123").email("a@b.co");
    params.name = Some(None);
    let v = serde_json::to_value(&params).unwrap();
    assert_eq!(
        v,
        json!({ "customer_id": "cus_123", "email": "a@b.co", "name": null })
    );

    let back: UpdateCustomerParams = serde_json::from_value(v).unwrap();
    assert_eq!(back.name, Some(None));
    assert_eq!(back.fingerprint, None);
}

#[tokio::test]
async fn check_fails_open_on_server_error() {
    let server = MockServer::start(vec![Reply::json(500, json!({ "message": "boom" }))]).await;
    let res = client(&server)
        .check(CheckParams::new("cus_123", "messages"))
        .await
        .unwrap();
    assert!(res.allowed);
    assert_eq!(res.customer_id, "");
    assert!(res.balance.is_none());
}

#[tokio::test]
async fn track_fails_open_when_unreachable() {
    // Nothing listens on this port.
    let autumn = Autumn::builder()
        .secret_key("k")
        .server_url("http://127.0.0.1:9")
        .build()
        .unwrap();
    let res = autumn
        .track(TrackParams::new("cus_123").feature_id("messages"))
        .await
        .unwrap();
    assert_eq!(res.value, 0.0);
}

#[tokio::test]
async fn fail_open_can_be_disabled() {
    let server = MockServer::start(vec![Reply::json(
        503,
        json!({ "message": "down", "code": "unavailable" }),
    )])
    .await;
    let autumn = Autumn::builder()
        .secret_key("k")
        .server_url(&server.url)
        .fail_open(false)
        .build()
        .unwrap();
    let err = autumn
        .check(CheckParams::new("cus_123", "messages"))
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(503));
    assert_eq!(err.code(), Some("unavailable"));
}

#[tokio::test]
async fn api_errors_carry_status_code_and_message() {
    let server = MockServer::start(vec![Reply::json(
        404,
        json!({ "message": "Customer cus_x not found", "code": "customer_not_found" }),
    )])
    .await;
    let err = client(&server)
        .customers()
        .get(GetCustomerParams::new("cus_x"))
        .await
        .unwrap_err();
    let Error::Api(api) = &err else {
        panic!("expected an API error, got {err:?}")
    };
    assert_eq!(api.status, 404);
    assert_eq!(api.code.as_deref(), Some("customer_not_found"));
    assert_eq!(api.message.as_deref(), Some("Customer cus_x not found"));
    assert!(err
        .to_string()
        .starts_with("API error occurred: Status 404. Body: "));
    assert_eq!(server.requests()[0].path, "/v1/customers.get");
}

#[tokio::test]
async fn invalid_response_bodies_are_validation_errors() {
    let server = MockServer::start(vec![Reply::json(200, json!({ "allowed": "maybe" }))]).await;
    let autumn = Autumn::builder()
        .secret_key("k")
        .server_url(&server.url)
        .fail_open(false)
        .build()
        .unwrap();
    let err = autumn.check(CheckParams::new("c", "f")).await.unwrap_err();
    assert!(matches!(err, Error::ResponseValidation(_)), "{err:?}");
}

#[tokio::test]
async fn retries_with_backoff() {
    let server = MockServer::start(vec![
        Reply::json(503, json!({})).header("retry-after", "0"),
        Reply::json(429, json!({})),
        Reply::json(200, json!({ "success": true })),
    ])
    .await;
    let retries = RetryConfig::Backoff {
        backoff: BackoffStrategy {
            initial_interval: Duration::from_millis(1),
            max_interval: Duration::from_millis(5),
            ..Default::default()
        },
        retry_connection_errors: false,
    };
    let res = client(&server)
        .with_options(RequestOptions::new().retries(retries))
        .balances()
        .finalize(FinalizeBalanceParams::new(
            "lock_1",
            FinalizeLockAction::Confirm,
            "cus_123",
        ))
        .await
        .unwrap();
    assert!(res.success);
    assert_eq!(server.requests().len(), 3);
}

#[tokio::test]
async fn no_retries_by_default() {
    let server = MockServer::start(vec![
        Reply::json(503, json!({})),
        Reply::json(200, json!({ "success": true })),
    ])
    .await;
    let err = client(&server)
        .balances()
        .finalize(FinalizeBalanceParams::new(
            "lock_1",
            "release".parse::<FinalizeLockAction>().unwrap(),
            "c",
        ))
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(503));
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn edge_is_tried_first_and_falls_back_when_down() {
    let edge = MockServer::start(vec![Reply::json(502, json!({}))]).await;
    let api = MockServer::start(vec![Reply::json(200, check_ok())]).await;
    let autumn = Autumn::builder()
        .secret_key("k")
        .server_url(&api.url)
        .edge_url(&edge.url)
        .build()
        .unwrap();

    let res = autumn
        .edge()
        .check(CheckParams::new("cus_123", "messages").lock(Lock::new("lock_1")))
        .await
        .unwrap();
    assert!(res.allowed);

    let (e, a) = (edge.requests(), api.requests());
    assert_eq!(e.len(), 1);
    assert_eq!(a.len(), 1);
    assert_eq!(e[0].path, "/v1/balances.check");
    // A locking check is a write: the same idempotency key goes to both.
    let key = e[0].header("idempotency-key").expect("idempotency key");
    assert_eq!(a[0].header("idempotency-key"), Some(key));
    assert_eq!(
        a[0].json()["lock"],
        json!({ "lock_id": "lock_1", "enabled": true })
    );
}

#[tokio::test]
async fn edge_answer_is_used_when_forwarded_or_ok() {
    let edge = MockServer::start(vec![Reply::json(200, check_ok())]).await;
    let api = MockServer::start(vec![]).await;
    let autumn = Autumn::builder()
        .secret_key("k")
        .server_url(&api.url)
        .edge_url(&edge.url)
        .use_edge(true)
        .build()
        .unwrap();
    autumn
        .check(CheckParams::new("cus_123", "messages"))
        .await
        .unwrap();
    assert_eq!(edge.requests().len(), 1);
    assert_eq!(api.requests().len(), 0);
    // Non-edge operations always go to the API.
    let api2 = MockServer::start(vec![Reply::json(
        200,
        json!({ "customer_id": "c", "value": 1, "balance": null }),
    )])
    .await;
    let autumn = Autumn::builder()
        .secret_key("k")
        .server_url(&api2.url)
        .edge_url(&edge.url)
        .use_edge(true)
        .build()
        .unwrap();
    autumn
        .track(TrackParams::new("c").feature_id("f"))
        .await
        .unwrap();
    assert_eq!(api2.requests().len(), 1);
    assert_eq!(edge.requests().len(), 1);
}

#[tokio::test]
async fn operations_without_a_body_send_none() {
    let server = MockServer::start(vec![Reply::json(200, json!({ "list": [] }))]).await;
    let _ = client(&server).features().list().await;
    let req = &server.requests()[0];
    assert_eq!(req.path, "/v1/features.list");
    assert_eq!(req.body, "");
    assert_eq!(req.header("content-type"), None);
}

#[tokio::test]
async fn per_call_headers_override() {
    let server = MockServer::start(vec![Reply::json(200, check_ok())]).await;
    client(&server)
        .with_options(
            RequestOptions::new()
                .idempotency_key("my-key")
                .header("x-api-version", "1.2"),
        )
        .check(CheckParams::new("c", "f"))
        .await
        .unwrap();
    let req = &server.requests()[0];
    assert_eq!(req.header("idempotency-key"), Some("my-key"));
    assert_eq!(req.header("x-api-version"), Some("1.2"));
}
