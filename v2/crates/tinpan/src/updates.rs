//! Checking for, offering and installing updates, as Tin Pan 1 does: a quiet check 10 seconds after
//! startup (when Settings allows it), which speaks only when there's a version you haven't declined; Help,
//! Check for updates always answers. Downloads are verified before anything is installed. On Windows the
//! helper swaps TinPan.exe and starts it again; elsewhere the release page opens, until those builds can
//! install themselves.

use std::path::PathBuf;
use std::time::Duration;

use tinpan_core::i18n::{t, tf};
use tinpan_core::update::{self, UpdateInfo, Version};
use wxdragon::prelude::*;

use crate::net::Command;
use crate::{App, dialogs, speech, with_app};

pub const STARTUP_CHECK_AFTER: Duration = Duration::from_secs(10);

pub fn current() -> Version {
    Version::parse(tinpan_core::version()).expect("the crate version parses")
}

/// Whether this copy can replace itself: on Windows, a TinPan.exe that isn't a development build; on Linux, one
/// running as an AppImage (which says where its file is).
fn can_install() -> bool {
    if cfg!(target_os = "linux") {
        return std::env::var_os("APPIMAGE").is_some();
    }
    cfg!(windows) && std::env::current_exe().is_ok_and(|exe| !exe.components().any(|c| c.as_os_str() == "target"))
}

fn open_page(url: &str) {
    if !wxdragon::utils::launch_default_browser(url, BrowserLaunchFlags::Default) {
        speech::say_error(&t("Couldn't open the browser. The release page is github.com/ernie96/tin-pan/releases."));
    }
}

/// The update offer: Yes, No, and Donate, which shows how to support Tin Pan and then asks again.
fn offer(frame: &Frame, message: &str, caption: &str) -> bool {
    dialogs::confirm_or(frame, message, caption, "_Donate...", crate::contact::show_donate)
}

impl App {
    /// At startup: says if the last update failed, then checks quietly after a moment, if Settings allows.
    pub(crate) fn after_startup_updates(&mut self) {
        if update::tidy_after_update() {
            speech::say_error(&t("The last update couldn't be installed. Help, Check for updates tries again."));
        }
        if self.config.current.update.check_at_startup {
            self.update_timer.start(STARTUP_CHECK_AFTER.as_millis() as i32, true);
        }
    }

    /// Help, Check for updates (manual), or the quiet one after startup.
    pub(crate) fn check_for_updates(&mut self, manual: bool) {
        if self.updating {
            if manual {
                speech::say(&t("Already checking for updates."));
            }
            return;
        }
        self.updating = true;
        if manual {
            speech::say(&t("Checking for updates."));
        }
        self.net.send(Command::CheckForUpdates { manual });
    }

    pub(crate) fn update_checked(&mut self, result: Result<Option<UpdateInfo>, String>, manual: bool) {
        let update = match result {
            Ok(Some(update)) => update,
            Ok(None) => {
                self.updating = false;
                if manual {
                    speech::say(&tf("Tin Pan is up to date, version {0}.", &[&current().short()]));
                }
                return;
            }
            Err(why) => {
                self.updating = false;
                log::warn!("Update: {why}");
                if manual {
                    speech::say_error(&why);
                }
                return;
            }
        };
        let version = update.version.short();
        if !manual && self.config.current.update.skipped_version == version {
            self.updating = false;
            return;
        }
        let notes = update::plain_text(&update::changes_only(&update.notes));
        let whats_new = if notes.is_empty() { String::new() } else { format!("{}\n{notes}\n\n", t("What's new:")) };
        let reminder = if manual { String::new() } else { format!("\n\n{}", t("If you choose No, Tin Pan won't remind you about this version. Help, Check for updates still finds it.")) };
        let intro = format!("{}\n\n{whats_new}", tf("Tin Pan {0} is available. You have {1}.", &[&version, &current().short()]));
        if !can_install() {
            // A development build, or a system this version can't install on yet: the release page instead.
            if offer(&self.frame, &format!("{intro}{}{reminder}", t("This copy of Tin Pan can't update itself. Open the release page?")), &t("Tin Pan update")) {
                open_page(&update.page_url);
            } else if !manual {
                self.skip_version(&version);
            }
            self.updating = false;
            return;
        }
        if !offer(&self.frame, &format!("{intro}{}{reminder}", t("Download and install it now? Tin Pan closes and opens again by itself.")), &t("Tin Pan update")) {
            if !manual {
                self.skip_version(&version);
            }
            self.updating = false;
            return;
        }
        speech::say_error(&tf("Downloading Tin Pan {0}.", &[&version]));
        self.net.send(Command::DownloadUpdate { update: Box::new(update) });
    }

    fn skip_version(&mut self, version: &str) {
        self.config.current.update.skipped_version = version.to_string();
        self.save_settings();
    }

    /// The verified download: the helper starts, and Tin Pan closes so it can be replaced.
    pub(crate) fn update_downloaded(&mut self, result: Result<(PathBuf, String), String>) {
        self.updating = false;
        let (file, version) = match result {
            Ok(done) => done,
            Err(why) => {
                speech::say_error(&why);
                return;
            }
        };
        if let Err(why) = start_install(&file) {
            log::error!("Update helper didn't start: {why}");
            speech::say_error(&t("Couldn't start the update."));
            return;
        }
        speech::say_error(&tf("Installing Tin Pan {0}. It opens again in a moment.", &[&version]));
        // Let that be heard, then close the normal way so the settings are saved; ship-shape's watchdog ends
        // the process if closing stalls, since the update can't install while Tin Pan runs.
        let frame = self.frame;
        let timer = Timer::new(&frame);
        timer.on_tick(move |_| {
            frame.close(true);
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_secs(15));
                std::process::exit(0);
            });
        });
        timer.start(1500, true);
        std::mem::forget(timer);
    }
}

#[cfg(windows)]
fn start_install(file: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let script = update::install_script(std::process::id(), file, &exe);
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(drop)
        .map_err(|e| e.to_string())
}

/// The AppImage runtime says where the running AppImage is; the program itself runs from a mount that goes away.
#[cfg(target_os = "linux")]
fn start_install(file: &std::path::Path) -> Result<(), String> {
    let current = std::env::var_os("APPIMAGE").ok_or("not running from an AppImage")?;
    let script = update::appimage_script(std::process::id(), file, std::path::Path::new(&current));
    std::process::Command::new("sh").arg("-c").arg(&script).spawn().map(drop).map_err(|e| e.to_string())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn start_install(_file: &std::path::Path) -> Result<(), String> {
    Err("installing updates isn't supported here yet".into())
}

/// After an update relaunched Tin Pan, its window comes to the front; Windows would otherwise open it behind
/// whatever you were doing (ship-shape's way).
pub fn bring_to_front_after_update(frame: &Frame) {
    if std::env::var_os(update::RELAUNCH_ENV).is_none() {
        return;
    }
    // SAFETY: only this thread runs yet; programs Tin Pan starts later mustn't inherit the mark.
    unsafe { std::env::remove_var(update::RELAUNCH_ENV) };
    bring_to_front(frame);
}

/// Brings Tin Pan's window in front of whatever is, which Windows only allows with a little help.
pub fn bring_to_front(frame: &Frame) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow};
        let hwnd = frame.get_handle();
        if hwnd.is_null() {
            return;
        }
        // SAFETY: plain Win32 calls on our own window, from the window's thread.
        unsafe {
            let ours = GetCurrentThreadId();
            let foreground = GetForegroundWindow();
            let theirs = if foreground.is_invalid() { 0 } else { GetWindowThreadProcessId(foreground, None) };
            let attached = theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, true).as_bool();
            let _ = SetForegroundWindow(HWND(hwnd));
            if attached {
                let _ = AttachThreadInput(ours, theirs, false);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = frame;
}

/// The timer that runs the startup check.
pub fn startup_timer(frame: &Frame) -> Timer<Frame> {
    let timer = Timer::new(frame);
    timer.on_tick(|_| {
        with_app(|app| {
            // The offer waits until the sign-in window is closed, so it can't land on top of it (Tin Pan 1.3.0).
            if app.sign_in_window.is_some() {
                app.update_timer.start(5_000, true);
            } else {
                app.check_for_updates(false);
            }
        });
    });
    timer
}
