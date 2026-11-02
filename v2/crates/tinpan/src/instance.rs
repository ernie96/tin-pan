//! One Tin Pan at a time: starting it again brings the open one to the front instead of opening a second,
//! which would play over the first and fight it for the settings. wxWidgets' own checker tells (a named mutex
//! on Windows, a lock file elsewhere), and a small local message asks the open Tin Pan to come forward.

use wxdragon::ipc::{IPCClient, IPCConnection, IPCServer};
use wxdragon::prelude::*;

use crate::{App, with_app};

const NAME: &str = "TinPan2.SingleInstance";
const TOPIC: &str = "tinpan";
const SHOW: &str = "show";

/// The service the open Tin Pan listens on: a name on Windows (DDE), a socket file in the settings folder elsewhere.
fn service() -> String {
    if cfg!(windows) { "TinPan2".to_string() } else { tinpan_core::config::settings_dir().join("tinpan2.sock").display().to_string() }
}

/// What starting found: this is the only Tin Pan (keep these alive while it runs), or another one was open,
/// which was asked to come forward (`asked` says whether that worked).
pub enum Start {
    First(Option<SingleInstanceChecker>, Option<IPCServer>),
    AnotherIsOpen { asked: bool },
}

/// Checks for an open Tin Pan; if there is one, asks it to come forward. Call before making any window.
pub fn start() -> Start {
    let Some(checker) = SingleInstanceChecker::new(NAME, None) else {
        // A check that can't be made doesn't stop Tin Pan from starting.
        log::warn!("The single-instance check couldn't be made; starting anyway");
        return Start::First(None, None);
    };
    if checker.is_another_running() {
        return Start::AnotherIsOpen { asked: ask_open_one_forward() };
    }
    let server = IPCServer::new(|_topic| {
        Some(
            IPCConnection::builder()
                .on_execute(|_topic, data, _format| {
                    if data.starts_with(SHOW.as_bytes()) {
                        with_app(App::bring_forward);
                    }
                    true
                })
                .build(),
        )
    });
    let listening = server.create(&service());
    if !listening {
        log::warn!("Couldn't listen for a second start; it will say Tin Pan is open instead of bringing it forward");
    }
    Start::First(Some(checker), listening.then_some(server))
}

/// Lets the open Tin Pan take the foreground (Windows only allows that when the app in front says so), then asks.
fn ask_open_one_forward() -> bool {
    #[cfg(windows)]
    // SAFETY: a plain Win32 call; ASFW_ANY lets whichever process comes forward next do so.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(u32::MAX);
    }
    let client = IPCClient::new();
    match client.make_connection("localhost", &service(), TOPIC) {
        Some(connection) => {
            let sent = connection.execute_string(SHOW);
            let _ = connection.disconnect();
            sent
        }
        None => false,
    }
}

impl App {
    /// Another start of Tin Pan asked for this one: the main window comes forward, restored if it was minimized.
    pub(crate) fn bring_forward(&mut self) {
        self.frame.iconize(false);
        self.frame.show(true);
        self.frame.raise();
        crate::updates::bring_to_front(&self.frame);
        self.list().set_focus();
    }
}
