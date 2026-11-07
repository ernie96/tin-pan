//! Tin Pan's small windows, as in Tin Pan 1's DialogService: a text to read line by line (details,
//! lyrics), a list of lines to arrow through (song information), and a yes-or-no question.
//! Escape or Close closes each one.

use tinpan_core::i18n::t;
use wxdragon::prelude::*;

/// A read-only text, with the caret at its start so the screen reader reads from the top.
pub fn show_document(parent: &impl WxWidget, title: &str, text: &str) {
    let dialog = Dialog::builder(parent, title).with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder).with_size(700, 550).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let label = StaticText::builder(&dialog).with_label(title).build();
    let body = TextCtrl::builder(&dialog)
        .with_value(&text.replace('\n', "\r\n"))
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::Rich2)
        .build();
    sizer.add(&label, 0, SizerFlag::All, 8);
    sizer.add(&body, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 8);
    add_close(&dialog, &sizer);
    body.set_insertion_point(0);
    body.set_focus();
    dialog.show_modal();
    dialog.destroy();
}

/// Lines to step through with the arrow keys, one fact on each.
pub fn show_lines(parent: &impl WxWidget, title: &str, lines: &[String]) {
    let dialog = Dialog::builder(parent, title).with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder).with_size(600, 450).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let label = StaticText::builder(&dialog).with_label(title).build();
    let list = ListBox::builder(&dialog).with_choices(lines.to_vec()).build();
    if !lines.is_empty() {
        list.set_selection(0_u32, true);
    }
    sizer.add(&label, 0, SizerFlag::All, 8);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 8);
    add_close(&dialog, &sizer);
    // Enter in the list does nothing, as in Tin Pan 1, rather than pressing Close.
    on_enter_in(&dialog, &list, || {});
    list.set_focus();
    dialog.show_modal();
    dialog.destroy();
}

/// Lines to choose from: Enter or a double click picks one; Escape or Close picks none.
pub fn pick(parent: &impl WxWidget, title: &str, lines: &[String]) -> Option<usize> {
    let dialog = Dialog::builder(parent, title).with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder).with_size(600, 450).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let label = StaticText::builder(&dialog).with_label(title).build();
    let list = ListBox::builder(&dialog).with_choices(lines.to_vec()).build();
    if !lines.is_empty() {
        list.set_selection(0_u32, true);
    }
    sizer.add(&label, 0, SizerFlag::All, 8);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 8);
    add_close(&dialog, &sizer);
    on_enter_in(&dialog, &list, move || dialog.end_modal(ID_OK));
    list.on_item_double_clicked(move |_| dialog.end_modal(ID_OK));
    list.set_focus();
    let chosen = (dialog.show_modal() == ID_OK).then(|| list.get_selection()).flatten().map(|i| i as usize);
    dialog.destroy();
    chosen
}

/// Lines under a short explanation, in a window that stays open: Enter or a double click runs `enter` on the
/// selected line; Escape or Close closes it.
pub fn show_choices(parent: &impl WxWidget, title: &str, intro: &str, lines: &[String], enter: impl Fn(usize) + 'static) {
    let dialog = Dialog::builder(parent, title).with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder).with_size(600, 400).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let label = StaticText::builder(&dialog).with_label(intro).build();
    label.wrap(560);
    let list = ListBox::builder(&dialog).with_choices(lines.to_vec()).build();
    if !lines.is_empty() {
        list.set_selection(0_u32, true);
    }
    sizer.add(&label, 0, SizerFlag::All, 8);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 8);
    add_close(&dialog, &sizer);
    let enter = std::rc::Rc::new(enter);
    let on_enter = enter.clone();
    on_enter_in(&dialog, &list, move || {
        if let Some(i) = list.get_selection() {
            on_enter(i as usize);
        }
    });
    list.on_item_double_clicked(move |_| {
        if let Some(i) = list.get_selection() {
            enter(i as usize);
        }
    });
    list.set_focus();
    dialog.show_modal();
    dialog.destroy();
}

/// Runs `enter` when Enter is pressed in `list`. A dialog on Windows hands Enter to its buttons before the
/// list sees it, so it's caught by the dialog's key hook, which comes first.
fn on_enter_in(dialog: &Dialog, list: &ListBox, enter: impl Fn() + 'static) {
    let list = *list;
    dialog.on_char_hook(move |event| {
        if let WindowEventData::Keyboard(ref key) = event
            && matches!(key.get_key_code(), Some(WXK_RETURN | WXK_NUMPAD_ENTER))
            && !key.control_down()
            && !key.alt_down()
            && list.has_focus()
        {
            enter();
            event.skip(false);
        } else {
            event.skip(true);
        }
    });
}

fn add_close(dialog: &Dialog, sizer: &BoxSizer) {
    // Tin Pan 1's text; Escape is its key, as there.
    let close = Button::builder(dialog).with_id(ID_CANCEL).with_label(&t("Close")).build();
    sizer.add(&close, 0, SizerFlag::AlignRight | SizerFlag::All, 8);
    dialog.set_sizer(*sizer, true);
    dialog.set_escape_id(ID_CANCEL);
}

/// Asks a yes-or-no question with a third button beside them that runs `other` and asks again; true for Yes.
/// The question is a read-only text, so long release notes can be read line by line; Enter in it says Yes.
pub fn confirm_or(parent: &impl WxWidget, message: &str, caption: &str, other_label: &str, other: impl Fn(&Dialog)) -> bool {
    const OTHER: i32 = 10_001;
    loop {
        let dialog = Dialog::builder(parent, caption).with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder).with_size(640, 480).build();
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        let body = TextCtrl::builder(&dialog)
            .with_value(&message.replace('\n', "\r\n"))
            .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::Rich2)
            .build();
        sizer.add(&body, 1, SizerFlag::Expand | SizerFlag::All, 8);
        let buttons = BoxSizer::builder(Orientation::Horizontal).build();
        let yes = Button::builder(&dialog).with_id(ID_YES).with_label(&crate::form::wx_label("_Yes")).build();
        let no = Button::builder(&dialog).with_id(ID_NO).with_label(&crate::form::wx_label("_No")).build();
        let more = Button::builder(&dialog).with_id(OTHER).with_label(&crate::form::wx_label(other_label)).build();
        for button in [&yes, &no, &more] {
            buttons.add(button, 0, SizerFlag::All, 4);
        }
        sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 4);
        dialog.set_sizer(sizer, true);
        dialog.set_escape_id(ID_NO);
        yes.set_default();
        yes.on_click(move |_| dialog.end_modal(ID_YES));
        no.on_click(move |_| dialog.end_modal(ID_NO));
        more.on_click(move |_| dialog.end_modal(OTHER));
        // A multiline text keeps Enter for itself, so it's taken here first: Enter in the text says Yes.
        dialog.on_char_hook(move |event| {
            if let WindowEventData::Keyboard(ref key) = event
                && matches!(key.get_key_code(), Some(WXK_RETURN | WXK_NUMPAD_ENTER))
                && body.has_focus()
            {
                dialog.end_modal(ID_YES);
                event.skip(false);
            } else {
                event.skip(true);
            }
        });
        body.set_insertion_point(0);
        body.set_focus();
        let answer = dialog.show_modal();
        if answer == OTHER {
            other(&dialog);
            dialog.destroy();
            continue;
        }
        dialog.destroy();
        return answer == ID_YES;
    }
}

/// Asks a yes-or-no question; true for Yes.
pub fn confirm(parent: &impl WxWidget, message: &str, caption: &str) -> bool {
    let dialog = MessageDialog::builder(parent, message, caption).with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconQuestion).build();
    dialog.show_modal() == ID_YES
}
