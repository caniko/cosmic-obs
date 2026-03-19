mod helpers;

use screencast_portal_mock::{NormalSession, OldPortal};

#[tokio::test]
async fn v5_portal_reports_version_5() {
    let (_bus, _handle, client) = helpers::setup(OldPortal { version: 5 }).await;
    let version = helpers::get_version(&client).await;
    assert_eq!(version, 5);
}

#[tokio::test]
async fn v6_portal_reports_version_6() {
    let (_bus, _handle, client) = helpers::setup(NormalSession).await;
    let version = helpers::get_version(&client).await;
    assert_eq!(version, 6);
}

#[tokio::test]
#[ignore = "requires OBS"]
async fn obs_v5_portal_ignores_new_keys() {
    // Placeholder: would launch OBS against an OldPortal{version:5}
    // and verify it does not send source_label or restore_fail_policy.
}

#[tokio::test]
#[ignore = "requires OBS"]
async fn obs_v6_portal_sends_new_keys() {
    // Placeholder: would launch OBS against a NormalSession (version 6)
    // and verify it sends source_label and restore_fail_policy.
}
