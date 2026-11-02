//! What Tin Pan says. While Tin Pan is the window in front, a live region carries it, so the screen
//! reader reads it in its own voice and order; when it isn't (on Windows), it goes straight to the
//! screen reader through prism, since a live region in a background window goes unheard.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use live_region::Priority;
use tinpan_core::i18n::t;
use wxdragon::prelude::*;

/// Messages kept for Ctrl+Shift+R, as many as Tin Pan 1 keeps.
const HISTORY_SIZE: usize = 100;

thread_local! {
    static LABEL: Cell<Option<StaticText>> = const { Cell::new(None) };
    static LAST: RefCell<Option<String>> = const { RefCell::new(None) };
    static HISTORY: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
    /// Settings, Tin Pan speaks: 0 everything, 1 what you ask for and errors, 2 only errors.
    static LEVEL: Cell<u8> = const { Cell::new(0) };
}

/// The speech level from Settings: Everything, AskedFor or ErrorsOnly, as Tin Pan 1 keeps it.
pub fn set_level(level: &str) {
    LEVEL.set(match level {
        "AskedFor" => 1,
        "ErrorsOnly" => 2,
        _ => 0,
    });
}

/// Says an error, whatever the speech level; errors always need hearing.
pub fn say_error(text: &str) {
    announce(text, Priority::High, true);
}

/// Makes the hidden live-region label. Call once, on the window's thread, after the window is built.
pub fn init(parent: &impl WxWidget) {
    let label = StaticText::builder(parent).with_label("").with_size(Size::new(0, 0)).build();
    label.show(false);
    live_region::set_live_region(&label);
    LABEL.set(Some(label));
    #[cfg(all(windows, feature = "background-speech"))]
    background::init();
}

/// Says something the user just asked for, cutting off whatever is being said.
pub fn say(text: &str) {
    announce(text, Priority::High, LEVEL.get() < 2);
}

/// Says something that happened by itself (a song ended, a load finished), after what is being said.
pub fn say_quietly(text: &str) {
    announce(text, Priority::Medium, LEVEL.get() == 0);
}

/// What speech goes through, for About: the screen reader prism found, as Tin Pan 1 names it.
pub fn output_name() -> String {
    #[cfg(all(windows, feature = "background-speech"))]
    if let Some(name) = background::name() {
        let lower = name.to_lowercase();
        return if lower.contains("sapi") || lower.contains("onecore") { t("Windows speech") } else { name };
    }
    t("your screen reader")
}

/// Ctrl+R: says the last message again.
pub fn repeat_last() {
    let last = LAST.with_borrow(Clone::clone).unwrap_or_else(|| t("Nothing said yet."));
    speak(&last, Priority::High);
}

/// Ctrl+Shift+R: every message so far, newest first, each with its time.
pub fn history() -> Vec<String> {
    HISTORY.with_borrow(|h| h.iter().cloned().collect())
}

/// Every message goes into the history and Ctrl+R; `speak_it` says whether the speech level lets it be heard.
fn announce(text: &str, priority: Priority, speak_it: bool) {
    if text.trim().is_empty() {
        return;
    }
    LAST.set(Some(text.to_string()));
    HISTORY.with_borrow_mut(|h| {
        h.push_front(format!("{} {text}", chrono::Local::now().format("%H:%M")));
        h.truncate(HISTORY_SIZE);
    });
    if speak_it {
        speak(text, priority);
    }
}

fn speak(text: &str, priority: Priority) {
    #[cfg(all(windows, feature = "background-speech"))]
    if !background::tin_pan_in_front() {
        background::speak(text, priority == Priority::High);
        return;
    }
    if let Some(label) = LABEL.get() {
        live_region::announce_with_priority(label, text, priority);
    }
}

#[cfg(all(windows, feature = "background-speech"))]
mod background {
    use prismer::{Backend, Prism};

    thread_local! {
        // Prism is only used from the window's thread, and a backend isn't Send. A backend borrows its
        // context; leaking the context once lets the backend be kept rather than rebuilt for every line.
        static BACKEND: Option<Backend<'static>> = Prism::new().ok().and_then(|prism| {
            let prism: &'static Prism = Box::leak(Box::new(prism));
            prism.create_best().ok()
        });
    }

    pub fn init() {
        BACKEND.with(|_| {});
    }

    pub fn name() -> Option<String> {
        BACKEND.with(|backend| backend.as_ref().map(prismer::Backend::name)).filter(|n| !n.is_empty())
    }

    pub fn speak(text: &str, interrupt: bool) {
        BACKEND.with(|backend| {
            if let Some(backend) = backend {
                let _ = backend.speak(text, interrupt);
            }
        });
    }

    /// Whether the window in front belongs to Tin Pan (the main window or one of its dialogs).
    pub fn tin_pan_in_front() -> bool {
        use windows::Win32::System::Threading::GetCurrentProcessId;
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
        let mut process = 0;
        // SAFETY: plain Win32 queries; process is a valid out pointer.
        unsafe {
            let front = GetForegroundWindow();
            if front.is_invalid() {
                return false;
            }
            GetWindowThreadProcessId(front, Some(&raw mut process));
            process == GetCurrentProcessId()
        }
    }
}
