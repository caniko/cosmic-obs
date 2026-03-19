//! Basic example: spawn a private D-Bus, start the mock portal, and verify it responds.

use screencast_portal_mock::{MockPortal, NormalSession, PrivateBus};

#[tokio::main]
async fn main() {
    // 1. Spin up an isolated dbus-daemon
    let bus = PrivateBus::spawn()
        .await
        .expect("failed to spawn dbus-daemon");
    println!("Bus address: {}", bus.address());

    // 2. Start the mock ScreenCast portal with a normal-session scenario
    let handle = MockPortal::start(&bus, NormalSession)
        .await
        .expect("failed to start mock portal");

    // 3. At this point, a client could connect to bus.address() and call
    //    CreateSession / SelectSources / Start on org.freedesktop.portal.ScreenCast.
    //    The mock records every SelectSources call for later assertion.

    println!("Mock portal is running. No calls recorded yet.");
    handle.assert_call_count(0);
    println!("Done.");
}
