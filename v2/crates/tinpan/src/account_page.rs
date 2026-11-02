//! The Account tab (Ctrl+4), as Tin Pan 1's: your plan in full and what it includes, then your notifications,
//! unread ones saying "New" first. Its buttons are reached by their Alt letters, not by Tab.

use tinpan_core::discover::Notification;
use tinpan_core::i18n::{plural, t};
use wxdragon::prelude::*;

use crate::commands::Action;
use crate::form::wx_label;
use crate::net::Command;
use crate::pages::{command_button, heading};
use crate::{App, speech, with_app};

#[derive(Clone, Copy)]
pub struct AccountPage {
    pub panel: Panel,
    text: TextCtrl,
    notifications: ListBox,
}

pub fn build(notebook: &Notebook) -> AccountPage {
    let panel = Panel::builder(notebook).build();
    let title = heading(&panel, &t("Account"));
    let text = TextCtrl::builder(&panel)
        .with_value(&t("Not signed in."))
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::NoVScroll)
        .with_size(Size::new(-1, 110))
        .build();
    let refresh = command_button(&panel, &wx_label("_Refresh account"), Action::PlanText);
    let label = heading(&panel, &wx_label("_Notifications"));
    let refresh_notifications = Button::builder(&panel).with_label(&wx_label("Refresh n_otifications")).build();
    refresh_notifications.disable_focus_from_keyboard();
    refresh_notifications.on_click(|_| with_app(|app| app.load_notifications(true)));
    let mark = Button::builder(&panel).with_label(&wx_label("_Mark all as read")).build();
    mark.disable_focus_from_keyboard();
    mark.on_click(|_| with_app(App::mark_notifications_read));
    let notifications = ListBox::builder(&panel).build();

    let row = BoxSizer::builder(Orientation::Horizontal).build();
    row.add(&label, 0, SizerFlag::AlignCenterVertical, 0);
    row.add(&refresh_notifications, 0, SizerFlag::AlignCenterVertical | SizerFlag::Left, 10);
    row.add(&mark, 0, SizerFlag::AlignCenterVertical | SizerFlag::Left, 10);
    let s = BoxSizer::builder(Orientation::Vertical).build();
    s.add(&title, 0, SizerFlag::All, 10);
    s.add(&text, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 10);
    s.add(&refresh, 0, SizerFlag::All, 10);
    s.add_sizer(&row, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    s.add(&notifications, 1, SizerFlag::Expand | SizerFlag::All, 10);
    panel.set_sizer(s, true);
    AccountPage { panel, text, notifications }
}

impl AccountPage {
    pub fn focus(&self) {
        self.text.set_focus();
        self.text.set_insertion_point(0);
    }
}

impl App {
    /// The plan in full, and what it includes, as F2 and Shift+F2 last loaded it.
    pub(crate) fn show_account_text(&self) {
        let text = match &self.billing {
            Some(billing) => format!("{} {}", billing.subscription_text(self.personalization), billing.features_text()).trim().to_string(),
            None => t("Not signed in."),
        };
        self.account.text.set_value(&text);
    }

    pub(crate) fn load_notifications(&mut self, say: bool) {
        if !self.signed_in {
            if say {
                speech::say(&t("Not signed in."));
            }
            return;
        }
        self.net.send(Command::Notifications { say });
    }

    pub(crate) fn notifications_loaded(&mut self, list: Vec<Notification>, say: bool) {
        let list_box = self.account.notifications;
        list_box.clear();
        for n in &list {
            list_box.append(&n.spoken_text());
        }
        if !list.is_empty() {
            list_box.set_selection(0_u32, true);
        }
        self.notifications = list;
        if say {
            let n = self.notifications.len();
            speech::say(&if n == 0 { t("No notifications.") } else { plural(n as i64, "{0} notification.", "{0} notifications.") });
        }
    }

    pub(crate) fn mark_notifications_read(&mut self) {
        if self.signed_in {
            self.net.send(Command::MarkNotificationsRead);
        }
    }

    pub(crate) fn notifications_read(&mut self) {
        let mut list = std::mem::take(&mut self.notifications);
        for n in &mut list {
            n.is_read = true;
        }
        self.notifications_loaded(list, false);
        speech::say(&t("Notifications marked as read."));
    }

    /// Signing out empties the page.
    pub(crate) fn clear_account_page(&mut self) {
        self.notifications.clear();
        self.account.notifications.clear();
        self.show_account_text();
    }
}
