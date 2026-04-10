mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::OldPortal;

#[tokio::test]
async fn version_zero() {
    let (_bus, _handle, client) = helpers::setup(OldPortal { version: 0 }).await;

    let version = helpers::get_version(&client).await;
    assert_eq!(version, 0);

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, HashMap::new()).await;
    assert_eq!(resp, 0);

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0);
    assert!(results.contains_key("streams"));
}

#[tokio::test]
async fn version_four() {
    let (_bus, _handle, client) = helpers::setup(OldPortal { version: 4 }).await;

    let version = helpers::get_version(&client).await;
    assert_eq!(version, 4);

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, HashMap::new()).await;
    assert_eq!(resp, 0);

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0);
    assert!(results.contains_key("streams"));
}

#[tokio::test]
async fn version_ninety_nine() {
    let (_bus, _handle, client) = helpers::setup(OldPortal { version: 99 }).await;

    let version = helpers::get_version(&client).await;
    assert_eq!(version, 99);

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, HashMap::new()).await;
    assert_eq!(resp, 0);

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0);
    assert!(results.contains_key("streams"));
}
