//! Suno's own pages (the hidden player, and the captcha check until it needs you) stay out of reach, as in
//! Tin Pan 1.8.1: not in Alt+Tab or the taskbar, and they can't take the focus.

use wxdragon::prelude::*;

/// Keeps `frame` out of Alt+Tab and unable to take the focus (`hidden`), or makes it an ordinary window again.
pub fn set(frame: &Frame, hidden: bool) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WS_EX_APPWINDOW,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };
        let hwnd = HWND(frame.get_handle());
        // A tool window isn't in Alt+Tab; a no-activate one can't be given the focus.
        let flags = (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) as isize;
        // SAFETY: plain Win32 calls on the frame's own, live window handle.
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let changed = if hidden { (style | flags) & !(WS_EX_APPWINDOW.0 as isize) } else { style & !flags };
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, changed);
            let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
        }
    }
    #[cfg(not(windows))]
    let _ = (frame, hidden);
}

/// If the window gets the focus anyway, it goes back to the Tin Pan window you were in.
pub fn give_focus_back(frame: &Frame, main: Frame) {
    frame.on_activate(move |event| {
        if let WindowEventData::Activate(ref data) = event
            && data.is_active()
        {
            crate::tool_form::back_to_active(&main);
        }
        event.skip(true);
    });
}
