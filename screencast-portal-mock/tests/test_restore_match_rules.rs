mod helpers;

use std::collections::HashSet;

use screencast_portal_mock::obs::ObsClientModel;
use screencast_portal_mock::{
    ACCEPTED_SELECT_SOURCES_KEYS, NormalSession, RestoreAction, RestoreFailReason,
    RestoreMatchRule, RestoreMatchScope, RestoreTokenFails, RestoreTokenRecord,
};

#[tokio::test]
async fn obs_model_sends_default_skip_label_and_match_rules() {
    let aliases =
        "\nproject-a\nsame: project-b\nsame_app: project-c\nany: shared\nany_app: vault\n";
    let model = ObsClientModel::new("OBS Camera")
        .with_restore_token("restore-token")
        .with_restore_match_rules(aliases);

    let expected_rules = vec![
        RestoreMatchRule::title_regex("project-a", RestoreMatchScope::SameApp),
        RestoreMatchRule::title_regex("project-b", RestoreMatchScope::SameApp),
        RestoreMatchRule::title_regex("project-c", RestoreMatchScope::SameApp),
        RestoreMatchRule::title_regex("shared", RestoreMatchScope::AnyApp),
        RestoreMatchRule::title_regex("vault", RestoreMatchScope::AnyApp),
    ];
    assert_eq!(model.parsed_restore_match_rules(), expected_rules);

    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;
    let (resp, _) =
        helpers::select_sources(&client, &session, model.select_sources_options(6)).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    handle.assert_source_label(0, "OBS Camera");
    handle.assert_restore_default_action(0, RestoreAction::Skip);
    handle.assert_restore_match_rules(0, &expected_rules);
}

#[tokio::test]
async fn any_app_broad_alias_does_not_share_unrelated_window_without_prompt() {
    let model = ObsClientModel::new("OBS Window Capture")
        .with_restore_token("stale-token")
        .with_restore_match_rules("any_app: .");
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::SourceUnavailable,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let (resp, results) =
        helpers::select_sources(&client, &session, model.select_sources_options(6)).await;
    assert_eq!(
        resp, 0,
        "SelectSources should not surface source_unavailable"
    );
    assert!(!results.contains_key("streams"));

    let (resp, results) = helpers::start_session_and_wait_closed(&client, &session).await;
    assert_eq!(
        resp, 1,
        "default OBS Skip should cancel instead of returning an arbitrary stream"
    );
    assert!(
        !results.contains_key("streams"),
        "broad any_app alias must not silently share a window"
    );
    assert!(!results.contains_key("restore_failure"));

    handle.assert_call_count(1);
    handle.assert_restore_match_rules(
        0,
        &[RestoreMatchRule::title_regex(
            ".",
            RestoreMatchScope::AnyApp,
        )],
    );
}

#[test]
fn obs_sent_keys_are_accepted_by_mock() {
    let accepted = ACCEPTED_SELECT_SOURCES_KEYS
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let sent = ObsClientModel::representative_v6_keys();

    assert!(
        sent.is_subset(&accepted),
        "OBS model sent unsupported SelectSources keys: {:?}",
        sent.difference(&accepted).collect::<Vec<_>>()
    );
}

#[test]
fn token_v1_v2_have_empty_aliases_and_v3_preserves_rules() {
    let v1 = RestoreTokenRecord::legacy_v1(2);
    assert_eq!(v1.restore_match_rules, vec![Vec::new(), Vec::new()]);

    let v2 = RestoreTokenRecord::legacy_v2(1);
    assert_eq!(v2.restore_match_rules, vec![Vec::new()]);

    let rules = vec![vec![RestoreMatchRule::title_regex(
        "project",
        RestoreMatchScope::SameApp,
    )]];
    let v3 = RestoreTokenRecord::v3(rules.clone());
    assert_eq!(v3.restore_match_rules, rules);
}
