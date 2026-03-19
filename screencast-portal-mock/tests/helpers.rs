#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Duration;

use futures_lite::StreamExt;
use screencast_portal_mock::{MockPortal, MockPortalHandle, PrivateBus, Scenario};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

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

/// Get the version property from the ScreenCast interface.
pub async fn get_version(client: &zbus::Connection) -> u32 {
    let proxy = screencast_proxy(client).await;
    let version: OwnedValue = proxy
        .get_property("Version")
        .await
        .expect("get version property");
    u32::try_from(&version).expect("version is u32")
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
