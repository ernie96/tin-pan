//! A form that stays open, as Tin Pan 1's tool forms (Extend, Cover, Remaster) do: its button runs the job
//! and leaves the form as it is, so you can listen to the result, change something and run it again. Escape
//! or Close closes it. Controls are made in Tab order, each after its label, so NVDA names each one after
//! its label and the label's Alt letter moves to it.

use std::cell::Cell;
use std::rc::Rc;

use tinpan_core::i18n::{t, tf};
use wxdragon::prelude::*;

use crate::form::{parse_number, wx_label};

thread_local! {
    /// The forms open now, in the order they opened, for F6.
    static OPEN: std::cell::RefCell<Vec<Dialog>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The Tin Pan window last in front: None for the main window, or Create or a form.
    static ACTIVE: std::cell::Cell<Option<Dialog>> = const { std::cell::Cell::new(None) };
}

/// Keeps track of when this window comes to the front, so focus can go back to it (after the captcha).
pub fn track_active(window: &impl WindowEvents, dialog: Option<Dialog>) {
    window.on_activate(move |event| {
        if let WindowEventData::Activate(ref data) = event
            && data.is_active()
        {
            ACTIVE.set(dialog);
        }
        event.skip(true);
    });
}

/// Brings back the Tin Pan window that was in front: Create or a form if it's still open, else the main one.
pub fn back_to_active(main: &Frame) {
    match ACTIVE.get().filter(|d| d.is_valid() && d.is_shown()) {
        Some(dialog) => {
            dialog.raise();
            dialog.set_focus();
        }
        None => {
            main.raise();
            main.set_focus();
            crate::with_app(|app| app.list().set_focus());
        }
    }
}

/// The forms still open, oldest first.
pub fn open_forms() -> Vec<Dialog> {
    OPEN.with_borrow_mut(|open| {
        open.retain(|d| d.is_valid());
        open.clone()
    })
}

/// Puts focus on a control once the form is shown.
type Focus = Box<dyn Fn()>;

pub struct ToolForm {
    pub dialog: Dialog,
    parent: Frame,
    title: String,
    panel: ScrolledWindow,
    sizer: BoxSizer,
    /// The first control made, which gets focus when the form opens.
    first: Rc<Cell<Option<Focus>>>,
}

impl ToolForm {
    pub fn new(parent: &Frame, title: &str, intro: &str) -> Self {
        let dialog = Dialog::builder(parent, title).with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder).with_size(640, 700).build();
        let panel = ScrolledWindow::builder(&dialog).build();
        panel.set_scroll_rate(0, 20);
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        if !intro.trim().is_empty() {
            let about = StaticText::builder(&panel).with_label(&t("About this")).build();
            let text = TextCtrl::builder(&panel).with_value(intro).with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::NoVScroll).with_size(Size::new(-1, 70)).build();
            sizer.add(&about, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 12);
            sizer.add(&text, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 12);
        }
        Self { dialog, parent: *parent, title: title.to_string(), panel, sizer, first: Rc::new(Cell::new(None)) }
    }

    fn label(&self, label: &str) {
        let label = StaticText::builder(&self.panel).with_label(&wx_label(label)).build();
        self.sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    }

    fn focus_first(&self, focus: impl Fn() + 'static) {
        let earlier = self.first.take();
        self.first.set(Some(earlier.unwrap_or_else(|| Box::new(focus))));
    }

    /// A text box; more than one line makes it a multi-line box.
    pub fn text(&self, label: &str, value: &str, lines: i32) -> TextCtrl {
        self.label(label);
        let style = if lines > 1 { TextCtrlStyle::MultiLine } else { TextCtrlStyle::Default };
        let text = TextCtrl::builder(&self.panel).with_value(&value.replace('\n', "\r\n")).with_style(style).with_size(Size::new(-1, if lines > 1 { 22 * lines } else { -1 })).build();
        self.sizer.add(&text, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 6);
        self.focus_first(move || text.set_focus());
        text
    }

    /// A time box: seconds, or minutes and seconds like 2:30.
    pub fn time(&self, label: &str, seconds: Option<f64>) -> TextCtrl {
        let shown = seconds.map(|s| tinpan_core::clip::format_time(s.floor())).unwrap_or_default();
        let text = self.text(label, &shown, 1);
        text.set_min_size(Size::new(160, -1));
        text
    }

    pub fn choice(&self, label: &str, items: Vec<String>, selected: usize) -> Choice {
        self.label(label);
        let choice = Choice::builder(&self.panel).with_choices(items).build();
        choice.set_selection(selected as u32);
        self.sizer.add(&choice, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 6);
        self.focus_first(move || choice.set_focus());
        choice
    }

    pub fn check(&self, label: &str, value: bool) -> CheckBox {
        let check = CheckBox::builder(&self.panel).with_label(&wx_label(label)).build();
        check.set_value(value);
        self.sizer.add(&check, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
        self.focus_first(move || check.set_focus());
        check
    }

    /// A 0 to 100 slider, and after it a box saying Suno chooses (`suno_chooses`), ticked to start. Moving the
    /// slider unticks it, so the number is sent; ticking it again leaves the choice to Suno.
    pub fn slider(&self, label: &str, suno_chooses: &str, start: i32) -> (Slider, CheckBox) {
        self.label(label);
        let slider = Slider::builder(&self.panel).with_value(start).with_min_value(0).with_max_value(100).with_size(Size::new(300, -1)).build();
        let let_suno = CheckBox::builder(&self.panel).with_label(&t(suno_chooses)).build();
        let_suno.set_value(true);
        slider.on_slider(move |_| let_suno.set_value(false));
        let row = BoxSizer::builder(Orientation::Horizontal).build();
        row.add(&slider, 0, SizerFlag::AlignCenterVertical, 0);
        row.add(&let_suno, 0, SizerFlag::AlignCenterVertical | SizerFlag::Left, 12);
        self.sizer.add_sizer(&row, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 6);
        self.focus_first(move || slider.set_focus());
        (slider, let_suno)
    }

    /// A 0 to 100 slider on its own, starting at `start`.
    pub fn plain_slider(&self, label: &str, start: i32) -> Slider {
        self.ranged_slider(label, start, 0, 100)
    }

    /// A slider from `min` to `max` on its own, starting at `start`.
    pub fn ranged_slider(&self, label: &str, start: i32, min: i32, max: i32) -> Slider {
        self.label(label);
        let slider = Slider::builder(&self.panel).with_value(start).with_min_value(min).with_max_value(max).with_size(Size::new(300, -1)).build();
        self.sizer.add(&slider, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 6);
        self.focus_first(move || slider.set_focus());
        slider
    }

    pub fn button(&self, label: &str, run: impl Fn() + 'static) {
        let button = Button::builder(&self.panel).with_label(&wx_label(label)).build();
        button.on_click(move |_| run());
        self.sizer.add(&button, 0, SizerFlag::Left | SizerFlag::Top, 6);
    }

    /// Opens the form. `run` is its main button's job; the form stays open after it.
    pub fn show(self, ok_text: &str, run: impl Fn() + 'static) {
        let dialog = self.dialog;
        self.panel.set_sizer(self.sizer, true);
        let ok = Button::builder(&dialog).with_label(ok_text).build();
        let close = Button::builder(&dialog).with_id(ID_CANCEL).with_label(&t("Close")).build();
        // Enter in a field starts it, as Tin Pan 1's forms do.
        ok.set_default();
        ok.on_click(move |_| run());
        // Closing goes back to the main window, not to whatever Windows picks.
        let parent = self.parent;
        let closing = move || {
            dialog.destroy();
            parent.raise();
            crate::with_app(|app| app.list().set_focus());
        };
        close.on_click(move |_| closing());
        dialog.on_close(move |event| {
            closing();
            event.skip(false);
        });
        let buttons = BoxSizer::builder(Orientation::Horizontal).build();
        buttons.add(&ok, 0, SizerFlag::All, 8);
        buttons.add(&close, 0, SizerFlag::All, 8);
        let outer = BoxSizer::builder(Orientation::Vertical).build();
        outer.add(&self.panel, 1, SizerFlag::Expand, 0);
        outer.add_sizer(&buttons, 0, SizerFlag::AlignRight, 0);
        dialog.set_sizer(outer, true);
        dialog.set_escape_id(ID_CANCEL);
        // F6 and Shift+F6 move between Tin Pan's windows, F1 says what the form is and how it works (Tin Pan 1's
        // words), and the playback keys work in the form, so you can listen while you fine-tune.
        let help = format!(
            "{} {}",
            t("F6 moves between this window and the main window."),
            tf("{0}. Tab moves between the fields. {1} starts another with these settings, and this stays open to try again. F8 plays or pauses from here. Escape closes.", &[&self.title, &ok_text.replace('&', "")])
        );
        dialog.on_char_hook(move |event| {
            let WindowEventData::Keyboard(ref key) = event else { return };
            let Some(code) = key.get_key_code() else { return event.skip(true) };
            let plain = !key.control_down() && !key.alt_down();
            if code == WXK_F6 && plain {
                crate::with_app(|app| app.next_window_from(Some(dialog.get_handle()), key.shift_down()));
            } else if code == WXK_F1 && plain && !key.shift_down() {
                crate::speech::say_error(&help);
            } else if let Some(action) = crate::commands::playback_key(code, key.control_down(), key.shift_down(), key.alt_down()) {
                crate::with_app(|app| app.run_from_button(action));
            } else {
                event.skip(true);
                return;
            }
            event.skip(false);
        });
        track_active(&dialog, Some(dialog));
        OPEN.with_borrow_mut(|open| open.push(dialog));
        dialog.show(true);
        if let Some(focus) = self.first.take() {
            focus();
        }
    }
}

/// A 0 to 100 number from a slider, or None when Suno chooses.
pub fn percent(slider: &Slider, suno_chooses: &CheckBox) -> Option<f64> {
    (!suno_chooses.get_value()).then(|| f64::from(slider.get_value()))
}

/// A time box's seconds, or None when it isn't one.
pub fn seconds(text: &TextCtrl) -> Option<f64> {
    parse_number(text.get_value().trim(), true)
}
