//! The sign-in window: suno.com's own sign-in page. Once the page's browser holds a signed-in Clerk
//! "__client" cookie, the cookies go to the network thread to be tried, and the window closes when
//! they work. Same as Tin Pan 1, with the same browser profile on Windows, so a Google or Suno
//! sign-in saved there is remembered here too.

use std::cell::RefCell;
use std::rc::Rc;

use tinpan_core::i18n::t;
use wxdragon::prelude::*;
use wxdragon::widgets::{WebView, WebViewBackend};

/// __client lives on the Clerk domain, __session and ajs_anonymous_id on suno.com. Later sites win a
/// name clash; this order is the one sign-in worked with in Tin Pan 1.
pub(crate) const COOKIE_SITES: [&str; 3] = ["https://suno.com", "https://auth.suno.com", "https://clerk.suno.com"];

/// Suno's pages need Edge's WebView2 runtime on Windows; without it wxWidgets would use Internet Explorer,
/// which can't show them.
pub fn web_pages_available() -> bool {
    if cfg!(windows) && !WebView::is_backend_available(WebViewBackend::Edge) {
        log::warn!("The WebView2 runtime isn't installed");
        return false;
    }
    true
}

/// As web_pages_available, saying `why` (Tin Pan 1's words) when they can't work.
pub fn web_pages_work(why: &str) -> bool {
    let available = web_pages_available();
    if !available {
        crate::speech::say_error(why);
    }
    available
}

/// Points the page's browser at Tin Pan's own profile. Call before any web view exists.
pub fn prepare_browser_profile() {
    #[cfg(windows)]
    {
        let folder = dirs::data_local_dir().unwrap_or_default().join("Tin Pan").join("WebView2");
        // SAFETY: called at startup on the only thread there is, before anything reads the environment.
        unsafe {
            std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", folder);
            // Tin Pan 1 opens the same profile with this; WebView2 refuses two different sets at once.
            std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--autoplay-policy=no-user-gesture-required");
        }
    }
}

pub struct SignInWindow {
    frame: Frame,
    timer: Rc<Timer<Frame>>,
}

impl SignInWindow {
    /// Opens the window. `try_cookies` gets each new set of cookies that has a __client; `forget_suno`
    /// signs the page out of Suno first (after Sign out), leaving Google and other sites signed in.
    /// `closed` runs when the user closes the window without signing in.
    pub fn open(parent: &Frame, forget_suno: bool, try_cookies: impl Fn(String) + 'static, closed: impl Fn() + 'static) -> Self {
        let frame = Frame::builder().with_parent(parent).with_title(&t("Sign in to Suno")).with_size(Size::new(900, 750)).build();
        let view = WebView::builder(&frame).build();
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        sizer.add(&view, 1, SizerFlag::Expand, 0);
        frame.set_sizer(sizer, true);

        let state = Rc::new(Poll { view, last_tried: RefCell::new(None), busy: RefCell::new(false), forget_suno: RefCell::new(forget_suno) });
        let try_cookies: Rc<dyn Fn(String)> = Rc::new(try_cookies);
        let timer = Rc::new(Timer::new(&frame));
        timer.on_tick(move |_| state.tick(&try_cookies));
        timer.start(1500, false);
        let ticking = timer.clone();
        frame.on_close(move |event| {
            // The page is going; stop reading it before it's gone.
            ticking.stop();
            closed();
            event.skip(true);
        });

        view.load_url("https://suno.com/sign-in");
        frame.show(true);
        view.set_focus();
        Self { frame, timer }
    }

    /// Closes the window once the sign-in has worked.
    pub fn close(self) {
        self.timer.stop();
        self.frame.destroy();
    }
}

struct Poll {
    view: WebView,
    last_tried: RefCell<Option<String>>,
    busy: RefCell<bool>,
    forget_suno: RefCell<bool>,
}

impl Poll {
    fn tick(self: &Rc<Self>, try_cookies: &Rc<dyn Fn(String)>) {
        if *self.busy.borrow() {
            return;
        }
        *self.busy.borrow_mut() = true;
        let forget = std::mem::take(&mut *self.forget_suno.borrow_mut());
        let me = self.clone();
        let try_cookies = try_cookies.clone();
        browser::read_cookies(&self.view, &COOKIE_SITES, forget, move |jar| {
            *me.busy.borrow_mut() = false;
            let Some(jar) = jar else { return };
            if !jar.has_client() {
                return;
            }
            let header = jar.to_header();
            if me.last_tried.borrow().as_deref() == Some(header.as_str()) {
                return;
            }
            *me.last_tried.borrow_mut() = Some(header.clone());
            try_cookies(header);
        });
    }
}

#[cfg(windows)]
pub(crate) mod browser {
    use std::cell::RefCell;
    use std::rc::Rc;

    use tinpan_core::cookies::SunoCookies;
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2, ICoreWebView2_2, ICoreWebView2CookieList, ICoreWebView2CookieManager};
    use webview2_com::{GetCookiesCompletedHandler, take_pwstr};
    use windows::core::{HSTRING, Interface, PCWSTR, PWSTR};
    use wxdragon::widgets::WebView;

    /// The page's ICoreWebView2, once the Edge backend has started.
    pub fn core(view: &WebView) -> Option<ICoreWebView2> {
        let raw = view.get_native_backend();
        if raw.is_null() {
            return None; // still starting
        }
        // SAFETY: on Windows, wxWebView's Edge backend hands out its ICoreWebView2; borrowed, then AddRef'd by cloned().
        Some(unsafe { ICoreWebView2::from_raw_borrowed(&raw) }?.clone())
    }

    fn cookie_manager(view: &WebView) -> Option<ICoreWebView2CookieManager> {
        // SAFETY: plain COM calls on the window's thread.
        unsafe { core(view)?.cast::<ICoreWebView2_2>().ok()?.CookieManager().ok() }
    }

    /// A page learns the audio outputs' names only with the microphone permission, so Suno's player gets it while
    /// an output other than the default is chosen, as in Tin Pan 1. The page is never let record: its start-up
    /// script turns getUserMedia off.
    pub fn allow_device_names(view: &WebView, allow: bool) {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, COREWEBVIEW2_PERMISSION_STATE_ALLOW, COREWEBVIEW2_PERMISSION_STATE_DEFAULT, ICoreWebView2_13, ICoreWebView2Profile4,
        };
        use webview2_com::SetPermissionStateCompletedHandler;
        let Some(core) = core(view) else { return };
        // SAFETY: COM calls on the window's thread; the origin string outlives the call.
        let done = unsafe {
            let Ok(profile) = core.cast::<ICoreWebView2_13>().and_then(|c| c.Profile()).and_then(|p| p.cast::<ICoreWebView2Profile4>()) else {
                log::warn!("Couldn't let the player see the audio outputs: no WebView2 profile");
                return;
            };
            let origin = HSTRING::from("https://suno.com");
            let state = if allow { COREWEBVIEW2_PERMISSION_STATE_ALLOW } else { COREWEBVIEW2_PERMISSION_STATE_DEFAULT };
            let handler = SetPermissionStateCompletedHandler::create(Box::new(|_| Ok(())));
            profile.SetPermissionState(COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, PCWSTR(origin.as_ptr()), state, &handler)
        };
        if let Err(e) = done {
            log::warn!("Couldn't let the player see the audio outputs: {e}");
        }
    }

    /// Reads the cookies of `sites` into one jar and hands it to `done` (None if the browser isn't ready).
    /// With `forget_suno`, deletes those sites' cookies instead and hands back an empty jar.
    pub fn read_cookies(view: &WebView, sites: &[&str], forget_suno: bool, done: impl FnOnce(Option<SunoCookies>) + 'static) {
        let Some(manager) = cookie_manager(view) else {
            done(None);
            return;
        };
        let jar = Rc::new(RefCell::new(SunoCookies::default()));
        let remaining = Rc::new(RefCell::new(sites.len()));
        let done = Rc::new(RefCell::new(Some(done)));
        for site in sites {
            let (into, left, finish, deleter) = (jar.clone(), remaining.clone(), done.clone(), manager.clone());
            let handler = GetCookiesCompletedHandler::create(Box::new(move |result, list: Option<ICoreWebView2CookieList>| {
                if let (Ok(()), Some(list)) = (result, list) {
                    collect(&list, &mut into.borrow_mut(), forget_suno.then_some(&deleter));
                }
                *left.borrow_mut() -= 1;
                if *left.borrow() == 0
                    && let Some(finish) = finish.borrow_mut().take()
                {
                    finish(Some(if forget_suno { SunoCookies::default() } else { into.borrow().clone() }));
                }
                Ok(())
            }));
            let uri = HSTRING::from(*site);
            // SAFETY: uri outlives the call; the handler is called back on this thread.
            let sent = unsafe { manager.GetCookies(PCWSTR(uri.as_ptr()), &handler) };
            if sent.is_err() {
                log::warn!("Couldn't ask the sign-in page for its cookies");
                *remaining.borrow_mut() -= 1;
            }
        }
        // Every request that couldn't be sent has been counted; if none is still out, finish now.
        if *remaining.borrow() == 0
            && let Some(done) = done.borrow_mut().take()
        {
            done(Some(jar.borrow().clone()));
        }
    }

    fn collect(list: &ICoreWebView2CookieList, jar: &mut SunoCookies, delete_with: Option<&ICoreWebView2CookieManager>) {
        let mut count = 0u32;
        // SAFETY: COM calls with valid out pointers; strings come back CoTaskMem-allocated and take_pwstr frees them.
        unsafe {
            if list.Count(&mut count).is_err() {
                return;
            }
            for i in 0..count {
                let Ok(cookie) = list.GetValueAtIndex(i) else { continue };
                if let Some(manager) = delete_with {
                    let _ = manager.DeleteCookie(&cookie);
                    continue;
                }
                let (mut name, mut value) = (PWSTR::null(), PWSTR::null());
                if cookie.Name(&mut name).is_ok() && cookie.Value(&mut value).is_ok() {
                    jar.set(&take_pwstr(name), &take_pwstr(value));
                }
            }
        }
    }
}

/// On Linux the sign-in page is WebKitGTK's (what wxWebView uses there): its cookie manager is asked for each
/// site's cookies, as WebView2's is on Windows. The calls are WebKitGTK 4.1's and libsoup 3's own.
#[cfg(target_os = "linux")]
pub(crate) mod browser {
    use std::cell::RefCell;
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::rc::Rc;

    use tinpan_core::cookies::SunoCookies;
    use wxdragon::widgets::WebView;

    type Callback = unsafe extern "C" fn(source: *mut c_void, result: *mut c_void, data: *mut c_void);

    #[link(name = "webkit2gtk-4.1")]
    unsafe extern "C" {
        fn webkit_web_view_get_context(view: *mut c_void) -> *mut c_void;
        fn webkit_web_context_get_cookie_manager(context: *mut c_void) -> *mut c_void;
        fn webkit_cookie_manager_get_cookies(manager: *mut c_void, uri: *const c_char, cancellable: *mut c_void, callback: Callback, data: *mut c_void);
        fn webkit_cookie_manager_get_cookies_finish(manager: *mut c_void, result: *mut c_void, error: *mut *mut c_void) -> *mut GList;
        fn webkit_cookie_manager_delete_cookie(manager: *mut c_void, cookie: *mut c_void, cancellable: *mut c_void, callback: Option<Callback>, data: *mut c_void);
    }

    #[link(name = "soup-3.0")]
    unsafe extern "C" {
        fn soup_cookie_get_name(cookie: *mut c_void) -> *const c_char;
        fn soup_cookie_get_value(cookie: *mut c_void) -> *const c_char;
        fn soup_cookie_free(cookie: *mut c_void);
    }

    #[link(name = "glib-2.0")]
    unsafe extern "C" {
        fn g_list_free(list: *mut GList);
        fn g_error_free(error: *mut c_void);
    }

    #[repr(C)]
    struct GList {
        data: *mut c_void,
        next: *mut GList,
        prev: *mut GList,
    }

    /// Every site's answer goes into one jar; the last one hands it on.
    struct Reading {
        jar: SunoCookies,
        remaining: usize,
        forget: bool,
        manager: *mut c_void,
        done: Option<Box<dyn FnOnce(Option<SunoCookies>)>>,
    }

    fn cookie_manager(view: &WebView) -> Option<*mut c_void> {
        let raw = view.get_native_backend();
        if raw.is_null() {
            return None;
        }
        // SAFETY: on GTK, wxWebView's backend is its WebKitWebView; the context and its manager live as long as it.
        let manager = unsafe { webkit_web_context_get_cookie_manager(webkit_web_view_get_context(raw)) };
        (!manager.is_null()).then_some(manager)
    }

    unsafe extern "C" fn got_cookies(_source: *mut c_void, result: *mut c_void, data: *mut c_void) {
        // SAFETY: `data` is the Rc boxed for this one request in read_cookies, taken back exactly once here; this
        // runs on the window's thread, from GLib's main loop, as every callback does.
        let reading = unsafe { Box::from_raw(data.cast::<Rc<RefCell<Reading>>>()) };
        let mut state = reading.borrow_mut();
        let mut error = std::ptr::null_mut();
        // SAFETY: the list and its cookies are ours to read and free, as WebKitGTK documents.
        unsafe {
            let list = webkit_cookie_manager_get_cookies_finish(state.manager, result, &mut error);
            if !error.is_null() {
                g_error_free(error);
            }
            let mut item = list;
            while !item.is_null() {
                let cookie = (*item).data;
                if state.forget {
                    // Deleting finishes later, with the cookie; it's left to WebKitGTK rather than freed here.
                    webkit_cookie_manager_delete_cookie(state.manager, cookie, std::ptr::null_mut(), None, std::ptr::null_mut());
                    item = (*item).next;
                    continue;
                } else {
                    let (name, value) = (soup_cookie_get_name(cookie), soup_cookie_get_value(cookie));
                    if !name.is_null() && !value.is_null() {
                        let (name, value) = (CStr::from_ptr(name).to_string_lossy().to_string(), CStr::from_ptr(value).to_string_lossy().to_string());
                        state.jar.set(&name, &value);
                    }
                }
                soup_cookie_free(cookie);
                item = (*item).next;
            }
            if !list.is_null() {
                g_list_free(list);
            }
        }
        state.remaining -= 1;
        if state.remaining == 0
            && let Some(done) = state.done.take()
        {
            let jar = if state.forget { SunoCookies::default() } else { std::mem::take(&mut state.jar) };
            drop(state);
            done(Some(jar));
        }
    }

    /// Reads the cookies of `sites` into one jar and hands it to `done` (None if the page isn't ready). With
    /// `forget_suno`, deletes those sites' cookies instead and hands back an empty jar.
    pub fn read_cookies(view: &WebView, sites: &[&str], forget_suno: bool, done: impl FnOnce(Option<SunoCookies>) + 'static) {
        let Some(manager) = cookie_manager(view) else {
            done(None);
            return;
        };
        let reading = Rc::new(RefCell::new(Reading { jar: SunoCookies::default(), remaining: sites.len(), forget: forget_suno, manager, done: Some(Box::new(done)) }));
        for site in sites {
            let Ok(uri) = CString::new(*site) else { continue };
            let data = Box::into_raw(Box::new(reading.clone())).cast::<c_void>();
            // SAFETY: the manager is alive with the view; the URI is copied by WebKitGTK; got_cookies takes `data` back.
            unsafe { webkit_cookie_manager_get_cookies(manager, uri.as_ptr(), std::ptr::null_mut(), got_cookies, data) };
        }
    }

    /// The page can see the outputs' names without asking on Linux; nothing to grant.
    pub fn allow_device_names(_view: &WebView, _allow: bool) {}
}

/// On macOS the sign-in page is WebKit's WKWebView (what wxWebView uses there): its website data store's cookie
/// store is asked for its cookies, as WebView2's and WebKitGTK's are elsewhere.
#[cfg(target_os = "macos")]
pub(crate) mod browser {
    use std::cell::RefCell;
    use std::ffi::{CStr, c_char};

    use block::ConcreteBlock;
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use tinpan_core::cookies::SunoCookies;
    use wxdragon::widgets::WebView;

    /// The page can see the outputs' names without asking on macOS; nothing to grant.
    pub fn allow_device_names(_view: &WebView, _allow: bool) {}

    /// The page's WKHTTPCookieStore, once the web view is there.
    fn cookie_store(view: &WebView) -> Option<*mut Object> {
        let web_view = view.get_native_backend().cast::<Object>();
        if web_view.is_null() {
            return None;
        }
        // SAFETY: on macOS, wxWebView's backend is its WKWebView; these are its own getters, on the main thread.
        let store: *mut Object = unsafe {
            let configuration: *mut Object = msg_send![web_view, configuration];
            let data: *mut Object = msg_send![configuration, websiteDataStore];
            msg_send![data, httpCookieStore]
        };
        (!store.is_null()).then_some(store)
    }

    /// An NSString's text.
    ///
    /// # Safety
    /// `string` is null or a live NSString.
    unsafe fn text(string: *mut Object) -> String {
        if string.is_null() {
            return String::new();
        }
        // SAFETY: as the caller promises; UTF8String lives as long as the string, and it's copied at once.
        unsafe {
            let utf8: *const c_char = msg_send![string, UTF8String];
            if utf8.is_null() { String::new() } else { CStr::from_ptr(utf8).to_string_lossy().into_owned() }
        }
    }

    /// Reads Suno's cookies (every *.suno.com site, which `sites` all are) into one jar and hands it to `done`
    /// (None if the page isn't ready). With `forget_suno`, deletes them instead and hands back an empty jar.
    pub fn read_cookies(view: &WebView, _sites: &[&str], forget_suno: bool, done: impl FnOnce(Option<SunoCookies>) + 'static) {
        let Some(store) = cookie_store(view) else {
            done(None);
            return;
        };
        let done = RefCell::new(Some(done));
        let got = ConcreteBlock::new(move |cookies: *mut Object| {
            let Some(done) = done.borrow_mut().take() else { return };
            let mut jar = SunoCookies::default();
            // SAFETY: WebKit hands the block an NSArray of NSHTTPCookie, on the main thread; the store outlives it.
            unsafe {
                let count: usize = if cookies.is_null() { 0 } else { msg_send![cookies, count] };
                for i in 0..count {
                    let cookie: *mut Object = msg_send![cookies, objectAtIndex: i];
                    let domain = text(msg_send![cookie, domain]);
                    if !domain.trim_start_matches('.').ends_with("suno.com") {
                        continue;
                    }
                    if forget_suno {
                        let nothing: *mut Object = std::ptr::null_mut();
                        let _: () = msg_send![store, deleteCookie: cookie completionHandler: nothing];
                    } else {
                        jar.set(&text(msg_send![cookie, name]), &text(msg_send![cookie, value]));
                    }
                }
            }
            done(Some(if forget_suno { SunoCookies::default() } else { jar }));
        })
        .copy();
        // SAFETY: getAllCookies: copies the block and calls it once, on the main thread.
        unsafe {
            let _: () = msg_send![store, getAllCookies: &*got];
        }
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(crate) mod browser {
    use tinpan_core::cookies::SunoCookies;
    use wxdragon::widgets::WebView;

    pub fn allow_device_names(_view: &WebView, _allow: bool) {}

    pub fn read_cookies(_view: &WebView, _sites: &[&str], _forget_suno: bool, done: impl FnOnce(Option<SunoCookies>) + 'static) {
        done(None);
    }
}
