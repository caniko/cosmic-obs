#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Duration;

use futures_lite::StreamExt;
use screencast_portal_mock::{MockPortal, MockPortalHandle, PrivateBus, Scenario};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Str, Value};

/// Set up a private bus, start the mock portal, and return a client connection.
pub async fn setup<S: Scenario>(scenario: S) -> (PrivateBus, MockPortalHandle, zbus::Connection) {
    let bus = PrivateBus::spawn().await.expect("spawn bus");
    let handle = MockPortal::start(&bus, scenario)
        .await
        .expect("start mock portal");

    // Give the object server a moment to register
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Connect a client (without requesting the well-known name)
    let client = zbus::connection::Builder::address(bus.address())
        .expect("parse address")
        .build()
        .await
        .expect("client connect");

    (bus, handle, client)
}

/// Subscribe to all Response signals, then call `CreateSession`, then wait
/// for the Response with the matching request path.
pub async fn create_session(client: &zbus::Connection) -> OwnedObjectPath {
    let rule = response_match_rule();
    let mut stream = zbus::MessageStream::for_match_rule(rule, client, Some(64))
        .await
        .expect("create message stream");

    let proxy = screencast_proxy(client).await;
    let options: HashMap<String, OwnedValue> = HashMap::new();
    let request_path: OwnedObjectPath = proxy
        .call("CreateSession", &(options,))
        .await
        .expect("CreateSession call");

    let (response, results) = recv_response_for(&mut stream, &request_path).await;
    assert_eq!(response, 0, "CreateSession should succeed");

    let session_handle = results
        .get("session_handle")
        .expect("session_handle in results");
    let session_str: String = session_handle
        .downcast_ref::<zbus::zvariant::Str<'_>>()
        .expect("session_handle is a string")
        .to_string();
    OwnedObjectPath::try_from(session_str).expect("valid object path")
}

/// Subscribe, call `SelectSources`, wait for Response.
pub async fn select_sources(
    client: &zbus::Connection,
    session_handle: &OwnedObjectPath,
    extra_options: HashMap<String, OwnedValue>,
) -> (u32, HashMap<String, OwnedValue>) {
    let rule = response_match_rule();
    let mut stream = zbus::MessageStream::for_match_rule(rule, client, Some(64))
        .await
        .expect("create message stream");

    let proxy = screencast_proxy(client).await;
    let request_path: OwnedObjectPath = proxy
        .call("SelectSources", &(session_handle.clone(), extra_options))
        .await
        .expect("SelectSources call");

    recv_response_for(&mut stream, &request_path).await
}

/// Call `SelectSources`, wait for Response, then wait for Session::Closed.
pub async fn select_sources_and_wait_closed(
    client: &zbus::Connection,
    session_handle: &OwnedObjectPath,
    extra_options: HashMap<String, OwnedValue>,
) -> (u32, HashMap<String, OwnedValue>) {
    let response_rule = response_match_rule();
    let mut response_stream = zbus::MessageStream::for_match_rule(response_rule, client, Some(64))
        .await
        .expect("create response message stream");

    let closed_rule = session_closed_match_rule();
    let mut closed_stream = zbus::MessageStream::for_match_rule(closed_rule, client, Some(64))
        .await
        .expect("create session closed message stream");

    let proxy = screencast_proxy(client).await;
    let request_path: OwnedObjectPath = proxy
        .call("SelectSources", &(session_handle.clone(), extra_options))
        .await
        .expect("SelectSources call");

    let response = recv_response_for(&mut response_stream, &request_path).await;
    recv_session_closed_for(&mut closed_stream, session_handle).await;
    response
}

/// Call `SelectSources` and return the expected D-Bus error.
pub async fn select_sources_error(
    client: &zbus::Connection,
    session_handle: &OwnedObjectPath,
    extra_options: HashMap<String, OwnedValue>,
) -> zbus::Error {
    let proxy = screencast_proxy(client).await;
    proxy
        .call::<_, _, OwnedObjectPath>("SelectSources", &(session_handle.clone(), extra_options))
        .await
        .expect_err("SelectSources should fail")
}

/// Subscribe, call `Start`, wait for Response.
pub async fn start_session(
    client: &zbus::Connection,
    session_handle: &OwnedObjectPath,
) -> (u32, HashMap<String, OwnedValue>) {
    let rule = response_match_rule();
    let mut stream = zbus::MessageStream::for_match_rule(rule, client, Some(64))
        .await
        .expect("create message stream");

    let proxy = screencast_proxy(client).await;
    let options: HashMap<String, OwnedValue> = HashMap::new();
    let request_path: OwnedObjectPath = proxy
        .call("Start", &(session_handle.clone(), String::new(), options))
        .await
        .expect("Start call");

    recv_response_for(&mut stream, &request_path).await
}

/// Call `Start`, wait for Response, then wait for Session::Closed.
pub async fn start_session_and_wait_closed(
    client: &zbus::Connection,
    session_handle: &OwnedObjectPath,
) -> (u32, HashMap<String, OwnedValue>) {
    let response_rule = response_match_rule();
    let mut response_stream = zbus::MessageStream::for_match_rule(response_rule, client, Some(64))
        .await
        .expect("create response message stream");

    let closed_rule = session_closed_match_rule();
    let mut closed_stream = zbus::MessageStream::for_match_rule(closed_rule, client, Some(64))
        .await
        .expect("create session closed message stream");

    let proxy = screencast_proxy(client).await;
    let options: HashMap<String, OwnedValue> = HashMap::new();
    let request_path: OwnedObjectPath = proxy
        .call("Start", &(session_handle.clone(), String::new(), options))
        .await
        .expect("Start call");

    let response = recv_response_for(&mut response_stream, &request_path).await;
    recv_session_closed_for(&mut closed_stream, session_handle).await;
    response
}

/// Call `Start` and return the expected D-Bus error.
pub async fn start_session_error(
    client: &zbus::Connection,
    session_handle: &OwnedObjectPath,
) -> zbus::Error {
    let proxy = screencast_proxy(client).await;
    let options: HashMap<String, OwnedValue> = HashMap::new();
    proxy
        .call::<_, _, OwnedObjectPath>("Start", &(session_handle.clone(), String::new(), options))
        .await
        .expect_err("Start should fail")
}

/// Get the version property from the ScreenCast interface.
pub async fn get_version(client: &zbus::Connection) -> u32 {
    let proxy = screencast_proxy(client).await;
    let version: OwnedValue = proxy
        .get_property("Version")
        .await
        .expect("get version property");
    u32::try_from(&version).expect("version is u32")
}

pub fn restore_policy_value(default_action: Option<u32>, actions: &[(&str, u32)]) -> OwnedValue {
    let mut policy: HashMap<String, OwnedValue> = HashMap::new();
    if let Some(default_action) = default_action {
        policy.insert(
            "default_action".into(),
            OwnedValue::try_from(Value::U32(default_action)).unwrap(),
        );
    }

    if !actions.is_empty() {
        let actions: HashMap<String, u32> = actions
            .iter()
            .map(|(reason, action)| ((*reason).to_string(), *action))
            .collect();
        policy.insert("actions".into(), OwnedValue::from(actions));
    }

    OwnedValue::from(policy)
}

pub fn token_and_policy(
    default_action: Option<u32>,
    actions: &[(&str, u32)],
) -> HashMap<String, OwnedValue> {
    let mut opts = HashMap::new();
    opts.insert(
        "restore_token".into(),
        OwnedValue::try_from(Value::new("stale-token".to_string())).unwrap(),
    );
    opts.insert(
        "restore_policy".into(),
        restore_policy_value(default_action, actions),
    );
    opts
}

pub fn restore_failure(results: &HashMap<String, OwnedValue>) -> HashMap<String, OwnedValue> {
    results
        .get("restore_failure")
        .expect("restore_failure key should be present")
        .clone()
        .try_into()
        .expect("restore_failure should be a{sv}")
}

pub fn restore_failure_reason(failure: &HashMap<String, OwnedValue>) -> String {
    failure
        .get("reason")
        .expect("restore_failure.reason should be present")
        .downcast_ref::<Str<'_>>()
        .expect("restore_failure.reason should be string")
        .to_string()
}

pub fn restore_failure_action(failure: &HashMap<String, OwnedValue>) -> u32 {
    u32::try_from(
        failure
            .get("action")
            .expect("restore_failure.action should be present"),
    )
    .expect("restore_failure.action should be u32")
}

pub fn restore_failure_token_invalid(failure: &HashMap<String, OwnedValue>) -> bool {
    bool::try_from(
        failure
            .get("token_invalid")
            .expect("restore_failure.token_invalid should be present"),
    )
    .expect("restore_failure.token_invalid should be bool")
}

pub fn streams(results: &HashMap<String, OwnedValue>) -> Vec<(u32, HashMap<String, OwnedValue>)> {
    let value = results.get("streams").expect("streams should be present");
    let raw: Vec<OwnedValue> = value.clone().try_into().expect("streams should be av");
    raw.into_iter()
        .map(|stream| {
            let (node_id, props): (OwnedValue, OwnedValue) =
                stream.try_into().expect("stream should be (vv)");
            let node_id = u32::try_from(unvariant(node_id)).expect("stream node id should be u32");
            let props: Vec<OwnedValue> = unvariant(props)
                .try_into()
                .expect("stream props should be av");
            let props = props
                .into_iter()
                .map(|prop| {
                    let (key, value): (OwnedValue, OwnedValue) =
                        prop.try_into().expect("stream prop should be (vv)");
                    let key = unvariant(key)
                        .downcast_ref::<Str<'_>>()
                        .expect("stream prop key should be string")
                        .to_string();
                    (key, unvariant(value))
                })
                .collect();
            (node_id, props)
        })
        .collect()
}

fn unvariant(value: OwnedValue) -> OwnedValue {
    match Value::from(value) {
        Value::Value(inner) => OwnedValue::try_from(*inner).expect("variant should be ownable"),
        other => OwnedValue::try_from(other).expect("value should be ownable"),
    }
}

pub fn string_prop(props: &HashMap<String, OwnedValue>, key: &str) -> String {
    props
        .get(key)
        .unwrap_or_else(|| panic!("{key} should be present"))
        .downcast_ref::<Str<'_>>()
        .unwrap_or_else(|_| panic!("{key} should be string"))
        .to_string()
}

async fn screencast_proxy(client: &zbus::Connection) -> zbus::Proxy<'_> {
    zbus::Proxy::new(
        client,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.ScreenCast",
    )
    .await
    .expect("create proxy")
}

/// Build a match rule for any Response signal on the Request interface.
fn response_match_rule() -> zbus::MatchRule<'static> {
    zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.portal.Request")
        .unwrap()
        .member("Response")
        .unwrap()
        .build()
}

/// Build a match rule for any Closed signal on the Session interface.
fn session_closed_match_rule() -> zbus::MatchRule<'static> {
    zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.portal.Session")
        .unwrap()
        .member("Closed")
        .unwrap()
        .build()
}

/// Wait for a Response signal matching the given request path, with timeout.
async fn recv_response_for(
    stream: &mut zbus::MessageStream,
    request_path: &OwnedObjectPath,
) -> (u32, HashMap<String, OwnedValue>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline - tokio::time::Instant::now();
        let next = tokio::time::timeout(remaining, stream.next()).await;
        let msg = next
            .expect("response signal timed out")
            .expect("stream ended")
            .expect("message error");

        let header = msg.header();
        if let Some(path) = header.path() {
            if path.as_str() == request_path.as_str() {
                let body = msg.body();
                let (response, results): (u32, HashMap<String, OwnedValue>) =
                    body.deserialize().expect("deserialize Response body");
                return (response, results);
            }
        }
    }
}

/// Wait for a Closed signal matching the given session path, with timeout.
async fn recv_session_closed_for(stream: &mut zbus::MessageStream, session_path: &OwnedObjectPath) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline - tokio::time::Instant::now();
        let next = tokio::time::timeout(remaining, stream.next()).await;
        let msg = next
            .expect("session Closed signal timed out")
            .expect("stream ended")
            .expect("message error");

        let header = msg.header();
        if let Some(path) = header.path() {
            if path.as_str() == session_path.as_str() {
                return;
            }
        }
    }
}
