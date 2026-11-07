//! How to reach Tin Pan's author and support it: Help, Contact the author and Help, Donate, and the Donate
//! button in the update offer. The addresses live here only, never inside translated texts.

use tinpan_core::i18n::{t, tf};
use wxdragon::prelude::*;

use crate::{dialogs, speech};

pub const EMAIL: &str = "ernesto@melendez.land";
pub const MASTODON: &str = "ernie@someplace.social";
const MASTODON_URL: &str = "https://someplace.social/@ernie";
const GITHUB_URL: &str = "https://github.com/ernie96/tin-pan";
/// Ernesto's announcement channel for all their apps; posts name the app.
const TELEGRAM_URL: &str = "https://t.me/+-SHI1xTKJugwMzQ5";
const PAYPAL_URL: &str = "https://paypal.me/RefinnejLTD";
const KOFI_URL: &str = "https://ko-fi.com/ernie96";

/// A line to read and the address Enter opens.
struct Link {
    line: String,
    url: String,
}

fn contact_links() -> Vec<Link> {
    vec![
        Link { line: tf("Email: {0}", &[&EMAIL]), url: format!("mailto:{EMAIL}") },
        Link { line: tf("Mastodon: {0}", &[&MASTODON]), url: MASTODON_URL.to_string() },
        Link { line: tf("Telegram, news and updates for Ernesto's apps: {0}", &[&TELEGRAM_URL]), url: TELEGRAM_URL.to_string() },
        Link { line: tf("GitHub, Tin Pan's code and releases: {0}", &[&GITHUB_URL]), url: GITHUB_URL.to_string() },
    ]
}

fn donation_links() -> Vec<Link> {
    vec![
        Link { line: tf("PayPal: {0}", &[&PAYPAL_URL]), url: PAYPAL_URL.to_string() },
        Link { line: tf("Ko-fi: {0}", &[&KOFI_URL]), url: KOFI_URL.to_string() },
    ]
}

/// A list of links that stays open: Enter opens the selected one in the browser or email app.
fn show_links(parent: &impl WxWidget, title: &str, intro: &str, links: &[Link]) {
    let lines: Vec<String> = links.iter().map(|l| l.line.clone()).collect();
    let urls: Vec<String> = links.iter().map(|l| l.url.clone()).collect();
    dialogs::show_choices(parent, title, intro, &lines, move |i| {
        if !wxdragon::utils::launch_default_browser(&urls[i], BrowserLaunchFlags::Default) {
            speech::say_error(&tf("Couldn't open {0}.", &[&urls[i]]));
        }
    });
}

/// Help, Contact the author.
pub fn show_contact(parent: &impl WxWidget) {
    show_links(
        parent,
        &t("Contact the author"),
        &t("Tin Pan is made by Ernesto Melendez. Enter opens the selected address. For a problem with Tin Pan, Help, Report a problem sends it straight to Ernesto with the details needed."),
        &contact_links(),
    );
}

/// Help, Donate, and the update offer's Donate button.
pub fn show_donate(parent: &impl WxWidget) {
    show_links(
        parent,
        &t("Donate"),
        &t("Tin Pan is free. If it's useful to you, a donation helps keep it going. Enter opens the selected page in your browser."),
        &donation_links(),
    );
}
