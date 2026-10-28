//! Keeps the claims made in docs/models.md honest.

use autumn_sdk::models::{
    CheckParams, EventsAggregateParams, EventsAggregateParamsFeatureIdUnion, FinalizeLockAction,
    Lock, TrackParams, UpdateCustomerParams,
};
use serde_json::json;

#[test]
fn builders_and_raw_identifiers() {
    let mut params = CheckParams::new("cus_123", "messages");
    params.required_balance = Some(5.0);
    assert_eq!(
        serde_json::to_value(&params).unwrap(),
        json!({ "customer_id": "cus_123", "feature_id": "messages", "required_balance": 5.0 })
    );

    let track = TrackParams::new("cus_123").r#async(false);
    assert_eq!(serde_json::to_value(&track).unwrap()["async"], json!(false));
}

#[test]
fn nullable_fields() {
    let mut update = UpdateCustomerParams::new("cus_123").name("Jane");
    assert_eq!(update.name, Some(Some("Jane".to_string())));
    update.email = Some(None);
    assert_eq!(
        serde_json::to_value(&update).unwrap(),
        json!({ "customer_id": "cus_123", "name": "Jane", "email": null })
    );
}

#[test]
fn enums_and_unions() {
    let action: FinalizeLockAction = "confirm".parse().unwrap();
    assert_eq!(action.as_str(), "confirm");
    assert!("nope".parse::<FinalizeLockAction>().is_err());

    let one = EventsAggregateParams::new("messages");
    assert_eq!(
        serde_json::to_value(&one).unwrap()["feature_id"],
        json!("messages")
    );
    let many = EventsAggregateParams::new(EventsAggregateParamsFeatureIdUnion::StringArray(vec![
        "messages".into(),
        "tokens".into(),
    ]));
    assert_eq!(
        serde_json::to_value(&many).unwrap()["feature_id"],
        json!(["messages", "tokens"])
    );
}

#[test]
fn literals_are_filled_in() {
    assert_eq!(
        serde_json::to_value(Lock::new("job_42")).unwrap(),
        json!({ "lock_id": "job_42", "enabled": true })
    );
}

#[test]
fn open_enums_keep_unknown_values() {
    let res: autumn_sdk::models::CheckResponse = serde_json::from_value(json!({
        "allowed": true,
        "customer_id": "cus_123",
        "balance": null,
        "flag": null,
        "preview": {
            "scenario": "some_future_scenario",
            "title": "t",
            "message": "m",
            "feature_id": "f",
            "feature_name": "F",
            "products": []
        }
    }))
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(res.allowed);
    let scenario = res.preview.unwrap().scenario;
    assert!(scenario.is_unknown());
    assert_eq!(scenario.as_str(), "some_future_scenario");
}

#[test]
fn requests_from_json() {
    let params: CheckParams =
        serde_json::from_value(json!({ "customer_id": "cus_123", "feature_id": "messages" }))
            .unwrap();
    assert_eq!(params, CheckParams::new("cus_123", "messages"));
}
