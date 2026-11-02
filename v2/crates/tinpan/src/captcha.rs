//! Suno's captcha, passed the way Tin Pan 1 passes it: Suno's own widget is rendered on suno.com/create in a
//! web view, signed in as you (the same browser profile as the sign-in window), and run. Trusted accounts pass
//! without seeing anything; otherwise the "Suno check" window comes on screen with the check box to press.
//!
//! The page is read with a timer, not waited on, so the window stays responsive: load the page, wait for the
//! captcha script to be ready, run it, then read the token it leaves in `window.__tinpanCaptcha`.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use tinpan_core::i18n::t;
use wxdragon::prelude::*;
use wxdragon::widgets::WebView;

use crate::speech;

const HCAPTCHA_SITE_KEY: &str = "d65453de-3f1a-4aac-9366-a0f06e52b2ce";
const TURNSTILE_SITE_KEY: &str = "0x4AAAAAADI7xDNyj-3LcIbi";

/// What the check hands back: the token and its provider, or what to say.
type Done = Box<dyn FnOnce(Result<(String, i32), String>)>;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    Loading,
    WaitingForScript,
    Settling,
    Solving,
}

struct Check {
    frame: Frame,
    view: WebView,
    parent: Frame,
    /// 1 hCaptcha, 2 Turnstile.
    provider: i32,
    visible: bool,
    step: Step,
    since: Instant,
    done: Option<Done>,
}

/// Gets a token for `provider` (1 hCaptcha, 2 Turnstile) and hands it to `done`, or why it couldn't.
pub fn solve(parent: &Frame, provider: i32, done: impl FnOnce(Result<(String, i32), String>) + 'static) {
    speech::say_quietly(&t("Passing Suno's captcha first. This can take half a minute."));
    let provider = if provider == 2 { 2 } else { 1 };
    // Off screen and out of the taskbar until it's needed; it comes back to the middle if a box must be pressed.
    let frame = Frame::builder()
        .with_parent(parent)
        .with_title(&t("Suno check"))
        .with_size(Size::new(520, 640))
        .with_position(Point::new(-20_000, -20_000))
        .with_style(FrameStyle::Default | FrameStyle::NoTaskbar)
        .build();
    let view = WebView::builder(&frame).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(&view, 1, SizerFlag::Expand, 0);
    frame.set_sizer(sizer, true);
    // Out of Alt+Tab and the focus until a box needs pressing.
    crate::out_of_reach::set(&frame, true);
    frame.show(true);
    // The window took focus to be shown; give it back to the window you were in.
    crate::tool_form::back_to_active(parent);

    let check = Rc::new(RefCell::new(Check { frame, view, parent: *parent, provider, visible: false, step: Step::Loading, since: Instant::now(), done: Some(Box::new(done)) }));
    let timer = Rc::new(Timer::new(&check.borrow().frame));
    let (ticking, me) = (timer.clone(), check.clone());
    timer.on_tick(move |_| {
        // Running a page script lets events through while it waits, this timer's among them: skip those ticks.
        let Ok(mut check) = me.try_borrow_mut() else { return };
        if check.tick() {
            ticking.stop();
        }
    });
    timer.start(500, false);
    // Closing the window gives up on the check.
    let (closing, me) = (timer.clone(), check.clone());
    check.borrow().frame.on_close(move |event| {
        closing.stop();
        if let Ok(mut check) = me.try_borrow_mut()
            && let Some(done) = check.done.take()
        {
            done(Err(t("Suno's captcha wasn't passed, so no song was made. You can try again, or make this one on suno.com.")));
        }
        event.skip(true);
    });
    check.borrow().view.load_url("https://suno.com/create");
    // The timer lives as long as its window.
    std::mem::forget(timer);
}

impl Check {
    /// One look at the page; true once it's over.
    fn tick(&mut self) -> bool {
        let waited = self.since.elapsed();
        match self.step {
            Step::Loading => {
                if self.script("document.readyState").as_deref() == Some("complete") || waited > Duration::from_secs(30) {
                    if self.provider == 2 {
                        self.script(
                            "if (!window.turnstile && !document.querySelector('script[src*=\"challenges.cloudflare.com/turnstile\"]')) { const s = document.createElement('script'); s.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit'; document.head.appendChild(s); } 'ok'",
                        );
                    }
                    self.next(Step::WaitingForScript);
                }
            }
            Step::WaitingForScript => {
                let ready = if self.provider == 2 { "String(typeof turnstile !== 'undefined' && !!turnstile.render)" } else { "String(typeof hcaptcha !== 'undefined' && !!hcaptcha.render)" };
                if self.script(ready).as_deref() == Some("true") {
                    self.next(Step::Settling);
                } else if waited > Duration::from_secs(20) {
                    log::warn!("Captcha provider {} never loaded on suno.com/create", self.provider);
                    return self.failed();
                }
            }
            // Let the captcha script finish wiring itself up.
            Step::Settling if waited > Duration::from_secs(2) => {
                self.script("window.__tinpanCaptcha = ''; 'ok'");
                let solving = if self.provider == 2 { turnstile(self.visible) } else { hcaptcha(self.visible) };
                self.script(&solving);
                self.next(Step::Solving);
            }
            Step::Settling => {}
            Step::Solving => {
                let result = self.script("String(window.__tinpanCaptcha || '')").unwrap_or_default();
                if !result.is_empty() && !result.starts_with("ERR:") {
                    return self.finish(Ok((result, self.provider)));
                }
                let limit = Duration::from_secs(if self.visible { 180 } else { 20 });
                if result.starts_with("ERR:") || waited > limit {
                    log::warn!("Captcha provider {} ({}) failed: {result}", self.provider, if self.visible { "shown" } else { "invisible" });
                    return self.failed();
                }
            }
        }
        false
    }

    fn next(&mut self, step: Step) {
        self.step = step;
        self.since = Instant::now();
    }

    /// Runs a script and gives back its result as plain text.
    fn script(&self, js: &str) -> Option<String> {
        let out = self.view.run_script(js)?;
        Some(out.trim().trim_matches('"').to_string())
    }

    /// Not passed: the first, invisible try brings the window on screen for the check box; the second gives up.
    fn failed(&mut self) -> bool {
        if self.visible {
            return self.finish(Err(t("Suno's captcha wasn't passed, so no song was made. You can try again, or make this one on suno.com.")));
        }
        self.visible = true;
        crate::out_of_reach::set(&self.frame, false);
        self.frame.center_on_screen();
        self.frame.raise();
        self.view.set_focus();
        speech::say(&t("Suno wants you to confirm you're a person. A Suno check window is open; press the check box in it if there is one."));
        self.view.load_url("https://suno.com/create");
        self.next(Step::Loading);
        false
    }

    fn finish(&mut self, result: Result<(String, i32), String>) -> bool {
        // Focus goes back only if the check window had it.
        if self.visible {
            crate::tool_form::back_to_active(&self.parent);
        }
        if let Some(done) = self.done.take() {
            done(result);
        }
        self.frame.destroy();
        true
    }
}

fn hcaptcha(visible: bool) -> String {
    let (place, size) = if visible { ("position:fixed;top:40px;left:40px;z-index:2147483647;", "normal") } else { ("position:fixed;top:-9999px;left:-9999px;", "invisible") };
    format!(
        "(async () => {{ try {{ const div = document.createElement('div'); div.style.cssText = '{place}'; document.body.appendChild(div);
        const id = hcaptcha.render(div, {{ sitekey: '{HCAPTCHA_SITE_KEY}', size: '{size}', sentry: false,
          endpoint: 'https://hcaptcha-endpoint-prod.suno.com', assethost: 'https://hcaptcha-assets-prod.suno.com',
          imghost: 'https://hcaptcha-imgs-prod.suno.com', reportapi: 'https://hcaptcha-reportapi-prod.suno.com' }});
        const r = await hcaptcha.execute(id, {{ async: true }}); window.__tinpanCaptcha = (r && r.response) ? r.response : 'ERR:no response';
        }} catch (e) {{ window.__tinpanCaptcha = 'ERR:' + String(e); }} }})(); 'started'"
    )
}

fn turnstile(visible: bool) -> String {
    let (appearance, timeout) = if visible { ("always", 175_000) } else { ("interaction-only", 15_000) };
    format!(
        "(() => {{ const div = document.createElement('div'); div.style.cssText = 'position:fixed;top:40px;left:40px;z-index:2147483647;'; document.body.appendChild(div);
        let settled = false; const finish = v => {{ if (!settled) {{ settled = true; clearTimeout(timer); window.__tinpanCaptcha = v; }} }};
        const timer = setTimeout(() => finish('ERR:Turnstile timed out'), {timeout});
        try {{ const w = turnstile.render(div, {{ sitekey: '{TURNSTILE_SITE_KEY}', execution: 'execute', appearance: '{appearance}',
          callback: t => finish(t), 'error-callback': c => finish('ERR:Turnstile ' + c), 'timeout-callback': () => finish('ERR:Turnstile challenge timed out') }});
          turnstile.execute(w); }} catch (e) {{ finish('ERR:' + String(e)); }} }})(); 'started'"
    )
}
