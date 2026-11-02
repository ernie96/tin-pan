//! A plain dialog built from fields, as Tin Pan 1's FormWindow: an explanation first (a read-only box, so
//! it can be reached and reread with the keyboard), then a label and a control for each field, then OK and
//! Cancel. Focus starts on the first field; Escape cancels; a problem is named, and its field focused,
//! before the form closes. Labels are Tin Pan 1's, with "_" before the access key.

use std::rc::Rc;

use tinpan_core::i18n::{t, tf};
use wxdragon::prelude::*;

// Number, Check, Heading and Path were the old Settings window's; Settings is a tab now. They stay for the
// number and time forms still to port (Change speed, Crop, Fade), which Tin Pan 1 checks the same way.
#[allow(dead_code)]
pub enum Value {
    Text { value: String, multiline: bool, max_len: usize },
    /// A number, or a time in seconds that may be typed as minutes:seconds. Empty is None when allowed.
    Number { value: Option<f64>, min: f64, max: f64, allow_empty: bool, is_time: bool },
    Choice { items: Vec<String>, selected: usize },
    Check { value: bool },
    /// A section's title (the label), with no control.
    Heading,
    /// A folder, or a file when `files` names its kind and pattern ("MP3 or WAV songs", "*.mp3;*.wav"), typed
    /// or chosen with a button. The button's text, the picker's title and the kind are Tin Pan 1's.
    Path { value: String, files: Option<(&'static str, &'static str)>, button: &'static str, picker_title: &'static str },
}

pub struct Field {
    /// Tin Pan 1's label, with "_" before the access key.
    pub label: &'static str,
    pub value: Value,
}

#[allow(dead_code)]
impl Field {
    pub fn choice(label: &'static str, items: Vec<String>, selected: usize) -> Self {
        Self { label, value: Value::Choice { items, selected } }
    }

    pub fn selected(&self) -> usize {
        match self.value {
            Value::Choice { selected, .. } => selected,
            _ => 0,
        }
    }

    pub fn check(label: &'static str, value: bool) -> Self {
        Self { label, value: Value::Check { value } }
    }

    pub fn heading(label: &'static str) -> Self {
        Self { label, value: Value::Heading }
    }

    pub fn number(label: &'static str, value: f64, min: f64, max: f64) -> Self {
        Self { label, value: Value::Number { value: Some(value), min, max, allow_empty: false, is_time: false } }
    }

    /// The text of a text or path field.
    pub fn text(&self) -> String {
        match &self.value {
            Value::Text { value, .. } | Value::Path { value, .. } => value.clone(),
            _ => String::new(),
        }
    }

    pub fn checked(&self) -> bool {
        matches!(self.value, Value::Check { value: true })
    }

    pub fn number_value(&self) -> Option<f64> {
        match self.value {
            Value::Number { value, .. } => value,
            _ => None,
        }
    }

    fn spoken_label(&self) -> String {
        t(self.label).replace('_', "")
    }
}

/// "_Order" as a wxWidgets label: "&Order", a real "&" doubled.
pub fn wx_label(label: &str) -> String {
    t(label).replace('&', "&&").replacen('_', "&", 1)
}

enum Control {
    Text(TextCtrl),
    Choice(Choice),
    Check(CheckBox),
    None,
}

/// Shows the form; true when OK was pressed, with the fields holding their new values.
pub fn show(parent: &impl WxWidget, title: &str, intro: &str, ok_text: &str, fields: &mut [Field]) -> bool {
    let dialog = Dialog::builder(parent, title).with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder).with_size(560, 600).build();
    let panel = ScrolledWindow::builder(&dialog).build();
    panel.set_scroll_rate(0, 20);
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let mut first: Option<Box<dyn Fn()>> = None;

    if !intro.trim().is_empty() {
        let about = StaticText::builder(&panel).with_label(&t("About this")).build();
        let box_ = TextCtrl::builder(&panel)
            .with_value(intro)
            .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::NoVScroll)
            .with_size(Size::new(-1, 70))
            .build();
        sizer.add(&about, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 12);
        sizer.add(&box_, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 12);
    }

    let mut controls = Vec::new();
    for field in fields.iter() {
        let control = match &field.value {
            Value::Heading => {
                let heading = StaticText::builder(&panel).with_label(&t(field.label)).build();
                sizer.add(&heading, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 16);
                Control::None
            }
            Value::Path { value, files, button, picker_title } => {
                let label = StaticText::builder(&panel).with_label(&wx_label(field.label)).build();
                let text = TextCtrl::builder(&panel).with_value(value).build();
                let browse = Button::builder(&panel).with_label(&wx_label(button)).build();
                let row = BoxSizer::builder(Orientation::Horizontal).build();
                row.add(&text, 1, SizerFlag::Expand, 0);
                row.add(&browse, 0, SizerFlag::Left, 8);
                sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 12);
                sizer.add_sizer(&row, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 12);
                let title = t(picker_title);
                let wildcard = files.map(|(kind, pattern)| format!("{} ({pattern})|{pattern}", t(kind)));
                browse.on_click(move |_| {
                    let chosen = if let Some(wildcard) = &wildcard {
                        let picker = FileDialog::builder(&text).with_message(&title).with_wildcard(wildcard).with_style(FileDialogStyle::Open | FileDialogStyle::FileMustExist).build();
                        if picker.show_modal() == ID_OK { picker.get_path() } else { None }
                    } else {
                        let picker = DirDialog::builder(&text, &title, &text.get_value()).build();
                        if picker.show_modal() == ID_OK { picker.get_path() } else { None }
                    };
                    if let Some(path) = chosen {
                        text.set_value(&path);
                    }
                    text.set_focus();
                });
                Control::Text(text)
            }
            Value::Check { value } => {
                let check = CheckBox::builder(&panel).with_label(&wx_label(field.label)).build();
                check.set_value(*value);
                sizer.add(&check, 0, SizerFlag::All, 12);
                Control::Check(check)
            }
            value => {
                // NVDA names the control after the label made just before it; the label's "&" letter moves there.
                let label = StaticText::builder(&panel).with_label(&wx_label(field.label)).build();
                sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 12);
                match value {
                    Value::Choice { items, selected } => {
                        let choice = Choice::builder(&panel).with_choices(items.clone()).build();
                        choice.set_selection(*selected as u32);
                        sizer.add(&choice, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 12);
                        Control::Choice(choice)
                    }
                    Value::Text { value, multiline, .. } => {
                        let style = if *multiline { TextCtrlStyle::MultiLine } else { TextCtrlStyle::Default };
                        let text = TextCtrl::builder(&panel).with_value(value).with_style(style).with_size(Size::new(-1, if *multiline { 120 } else { -1 })).build();
                        sizer.add(&text, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 12);
                        Control::Text(text)
                    }
                    Value::Number { value, is_time, .. } => {
                        let shown = value.map(|v| if *is_time { minutes_seconds(v) } else { trim_number(v) }).unwrap_or_default();
                        let text = TextCtrl::builder(&panel).with_value(&shown).with_size(Size::new(160, -1)).build();
                        sizer.add(&text, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 12);
                        Control::Text(text)
                    }
                    Value::Check { .. } | Value::Heading | Value::Path { .. } => unreachable!(),
                }
            }
        };
        // Focus starts on the first field that has a control (a heading has none).
        if first.is_none() {
            first = match &control {
                Control::Text(c) => {
                    let c = *c;
                    Some(Box::new(move || c.set_focus()))
                }
                Control::Choice(c) => {
                    let c = *c;
                    Some(Box::new(move || c.set_focus()))
                }
                Control::Check(c) => {
                    let c = *c;
                    Some(Box::new(move || c.set_focus()))
                }
                Control::None => None,
            };
        }
        controls.push(control);
    }
    panel.set_sizer(sizer, true);

    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let ok = Button::builder(&dialog).with_id(ID_OK).with_label(ok_text).build();
    let cancel = Button::builder(&dialog).with_id(ID_CANCEL).with_label(&t("Cancel")).build();
    ok.set_default();
    buttons.add(&ok, 0, SizerFlag::All, 8);
    buttons.add(&cancel, 0, SizerFlag::All, 8);
    let outer = BoxSizer::builder(Orientation::Vertical).build();
    outer.add(&panel, 1, SizerFlag::Expand, 0);
    outer.add_sizer(&buttons, 0, SizerFlag::AlignRight, 0);
    dialog.set_sizer(outer, true);
    dialog.set_escape_id(ID_CANCEL);

    // OK checks every field first, as Tin Pan 1 does, and only then closes.
    let controls = Rc::new(controls);
    let checks: Vec<(String, Check)> = fields.iter().map(|f| (f.spoken_label(), Check::of(&f.value))).collect();
    {
        let controls = controls.clone();
        let title = title.to_string();
        ok.on_click(move |_| {
            for (i, (label, check)) in checks.iter().enumerate() {
                let Control::Text(text) = &controls[i] else { continue };
                if let Some(problem) = check.problem(label, &text.get_value()) {
                    MessageDialog::builder(&dialog, &problem, &title).with_style(MessageDialogStyle::OK | MessageDialogStyle::IconWarning).build().show_modal();
                    text.set_focus();
                    text.select_all();
                    return;
                }
            }
            dialog.end_modal(ID_OK);
        });
    }

    if let Some(focus) = &first {
        focus();
    }
    let accepted = dialog.show_modal() == ID_OK;
    if accepted {
        for (field, control) in fields.iter_mut().zip(controls.iter()) {
            match (&mut field.value, control) {
                (Value::Text { value, .. } | Value::Path { value, .. }, Control::Text(c)) => *value = c.get_value().trim().to_string(),
                (Value::Number { value, is_time, .. }, Control::Text(c)) => *value = parse_number(c.get_value().trim(), *is_time),
                (Value::Choice { selected, .. }, Control::Choice(c)) => *selected = c.get_selection().unwrap_or(0) as usize,
                (Value::Check { value }, Control::Check(c)) => *value = c.get_value(),
                _ => {}
            }
        }
    }
    dialog.destroy();
    accepted
}

/// What a text or number box must hold, from its field.
enum Check {
    None,
    Length(usize),
    Number { min: f64, max: f64, allow_empty: bool, is_time: bool },
}

impl Check {
    fn of(value: &Value) -> Self {
        match value {
            Value::Text { max_len, .. } if *max_len > 0 => Check::Length(*max_len),
            Value::Number { min, max, allow_empty, is_time, .. } => Check::Number { min: *min, max: *max, allow_empty: *allow_empty, is_time: *is_time },
            _ => Check::None,
        }
    }

    /// A short sentence about what's wrong, in Tin Pan 1's words; None when it's fine.
    fn problem(&self, label: &str, text: &str) -> Option<String> {
        let text = text.trim();
        match *self {
            Check::None => None,
            Check::Length(max) => {
                let length = text.chars().count();
                (length > max).then(|| tf("{0} is {1} characters. The limit is {2}.", &[&label, &length, &max]))
            }
            Check::Number { min, max, allow_empty, is_time } => {
                if text.is_empty() {
                    return (!allow_empty).then(|| tf("{0} is needed.", &[&label]));
                }
                let Some(value) = parse_number(text, is_time) else {
                    return Some(if is_time { tf("{0}: type seconds, or minutes and seconds like 2:30.", &[&label]) } else { tf("{0} must be a number.", &[&label]) });
                };
                if value < min || value > max {
                    return Some(if is_time {
                        tf("{0} must be between {1} and {2}.", &[&label, &minutes_seconds(min), &minutes_seconds(max)])
                    } else {
                        tf("{0} must be from {1} to {2}.", &[&label, &trim_number(min), &trim_number(max)])
                    });
                }
                None
            }
        }
    }
}

/// A number with "." or ","; for a time, also minutes:seconds.
pub fn parse_number(text: &str, is_time: bool) -> Option<f64> {
    if text.is_empty() {
        return None;
    }
    if is_time && let Some((m, s)) = text.split_once(':') {
        let minutes: u32 = m.trim().parse().ok()?;
        let seconds: f64 = s.trim().replace(',', ".").parse().ok()?;
        return (0.0..60.0).contains(&seconds).then_some(f64::from(minutes) * 60.0 + seconds);
    }
    text.replace(',', ".").parse().ok().filter(|v: &f64| v.is_finite())
}

/// Total minutes, so an hour reads 60:00 rather than 0:00.
fn minutes_seconds(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

fn trim_number(v: f64) -> String {
    let text = format!("{v:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_times_read_as_tin_pan_1_reads_them() {
        assert_eq!(parse_number("2:30", true), Some(150.0));
        assert_eq!(parse_number("1,5", false), Some(1.5));
        assert_eq!(parse_number("1:75", true), None);
        assert_eq!(parse_number("abc", false), None);
        assert_eq!((minutes_seconds(3600.0), trim_number(2.50)), ("60:00".into(), "2.5".into()));
    }

    #[test]
    fn problems_are_named_in_tin_pan_1s_words() {
        let number = Check::Number { min: 0.0, max: 100.0, allow_empty: false, is_time: false };
        assert_eq!(number.problem("Weirdness", ""), Some("Weirdness is needed.".into()));
        assert_eq!(number.problem("Weirdness", "x"), Some("Weirdness must be a number.".into()));
        assert_eq!(number.problem("Weirdness", "120"), Some("Weirdness must be from 0 to 100.".into()));
        assert_eq!(number.problem("Weirdness", "50"), None);
        assert_eq!(Check::Length(3).problem("Title", "long"), Some("Title is 4 characters. The limit is 3.".into()));
    }
}
