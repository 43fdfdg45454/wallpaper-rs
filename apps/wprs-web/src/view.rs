use crate::engine::{DISPLAY, policy, uri};
use crate::ffi::{Ptr, get, text};
use crate::glib::{NUL, connect, set_bool, string};
use crate::state::{VIEWS, View, by_web, engine, live, send};
use std::cell::{Cell, RefCell};
use std::ffi::c_int;
use std::rc::Rc;
use wprs_web::FromWeb;

const RELOADS: u32 = 3;
const STABLE: u32 = 60_000;
const SETTINGS: [(&str, bool); 12] = [
    ("enable-webrtc", false),
    ("enable-media-stream", false),
    ("enable-mediasource", true),
    ("javascript-can-open-windows-automatically", false),
    ("javascript-can-access-clipboard", false),
    ("allow-file-access-from-file-urls", false),
    ("allow-universal-access-from-file-urls", false),
    ("enable-page-cache", false),
    ("enable-developer-extras", false),
    ("media-playback-requires-user-gesture", false),
    ("enable-webgl", true),
    ("enable-html5-local-storage", true),
];

pub fn open(id: u32, (width, height): (u32, u32), scale: f64) {
    let (a, Some(engine)) = (get(), engine()) else { return };
    unsafe {
        let manager = (a.webkit_user_content_manager_new)();
        (a.webkit_user_content_manager_add_filter)(manager, crate::state::FILTER.with(Cell::get));
        (a.webkit_user_content_manager_register_script_message_handler)(manager, c"wprs".as_ptr(), NUL);
        connect(
            manager,
            "script-message-received::wprs",
            crate::script::message as *const (),
            id as usize as Ptr,
        );
        let csp = text(&policy());
        let policies =
            (a.g_object_new)((a.webkit_website_policies_get_type)(), c"autoplay".as_ptr(), 0 as c_int, NUL);
        let web = (a.g_object_new)(
            (a.webkit_web_view_get_type)(),
            c"display".as_ptr(),
            DISPLAY.with(Cell::get),
            c"web-context".as_ptr(),
            engine.context,
            c"network-session".as_ptr(),
            engine.session,
            c"user-content-manager".as_ptr(),
            manager,
            c"default-content-security-policy".as_ptr(),
            csp.as_ptr(),
            c"website-policies".as_ptr(),
            policies,
            NUL,
        );
        (a.g_object_unref)(policies);
        let settings = (a.webkit_web_view_get_settings)(web);
        SETTINGS.iter().for_each(|(name, on)| set_bool(settings, name, *on));
        let [r, g, b] = engine.start.background.map(|c| f64::from(c) / 255.0);
        (a.webkit_web_view_set_background_color)(web, &[r, g, b, 1.0]);
        (a.webkit_web_view_set_is_muted)(web, c_int::from(live(|l| l.settings.volume <= 0.0)));
        let signals: [(&str, *const ()); 8] = [
            ("decide-policy", decide as *const ()),
            ("permission-request", deny as *const ()),
            ("create", create as *const ()),
            ("script-dialog", handled as *const ()),
            ("show-notification", handled as *const ()),
            ("run-file-chooser", handled as *const ()),
            ("web-process-terminated", terminated as *const ()),
            ("load-changed", loaded as *const ()),
        ];
        signals.iter().for_each(|(name, f)| connect(web, name, *f, std::ptr::null_mut()));
        let wpe = (a.webkit_web_view_get_wpe_view)(web);
        let top = (a.wpe_view_get_toplevel)(wpe);
        if !top.is_null() {
            (a.wpe_toplevel_resize)(top, width as c_int, height as c_int);
            (a.wpe_toplevel_scale_changed)(top, scale.max(0.25));
        }
        let fps = Cell::new(live(|l| l.settings.fps).max(1));
        let view = View {
            id,
            web,
            wpe,
            manager,
            fps,
            last: Cell::new(None),
            visible: Cell::new(true),
            hidden_at: Cell::new(None),
            stopped: Cell::new(false),
            crashes: Cell::new(0),
            known: RefCell::new(Vec::new()),
            held: RefCell::default(),
            ring: RefCell::new(None),
            loaded: Cell::new(false),
            answered: Cell::new(std::time::Instant::now()),
        };
        let view = Rc::new(view);
        VIEWS.with(|v| v.borrow_mut().insert(id, view.clone()));
        load(id);
        crate::watch::beat(view);
    }
}

pub fn load(id: u32) {
    let Some(view) = crate::state::view(id) else { return };
    crate::script::install(id, view.manager);
    let entry = engine().map(|e| e.start.entry.clone()).unwrap_or_default();
    let address = text(&uri(&entry));
    view.answered.set(std::time::Instant::now());
    unsafe { (get().webkit_web_view_load_uri)(view.web, address.as_ptr()) };
}

unsafe extern "C" fn decide(_web: Ptr, decision: Ptr, kind: c_int, _data: Ptr) -> c_int {
    let a = get();
    let ours = |request: Ptr| unsafe {
        let own = format!("wpak://{}/", engine().map_or("", |e| e.start.id.as_str()));
        string((a.webkit_uri_request_get_uri)(request)).starts_with(&own)
    };
    unsafe {
        match kind {
            0 => {
                let action = (a.webkit_navigation_policy_decision_get_navigation_action)(decision);
                match ours((a.webkit_navigation_action_get_request)(action)) {
                    true => (a.webkit_policy_decision_use)(decision),
                    false => (a.webkit_policy_decision_ignore)(decision),
                }
            }
            1 => (a.webkit_policy_decision_ignore)(decision),
            _ => (a.webkit_policy_decision_use)(decision),
        }
    }
    1
}

unsafe extern "C" fn deny(_web: Ptr, request: Ptr, _data: Ptr) -> c_int {
    unsafe { (get().webkit_permission_request_deny)(request) };
    1
}

unsafe extern "C" fn create(_web: Ptr, _action: Ptr, _data: Ptr) -> Ptr {
    std::ptr::null_mut()
}

unsafe extern "C" fn handled(_web: Ptr, _what: Ptr, _data: Ptr) -> c_int {
    1
}

unsafe extern "C" fn terminated(web: Ptr, reason: c_int, _data: Ptr) {
    let Some(view) = by_web(web) else { return };
    if reason == 2 && view.stopped.get() {
        return;
    }
    let (id, crashes) = (view.id, view.crashes.get() + 1);
    view.crashes.set(crashes);
    view.loaded.set(false);
    send(&FromWeb::Crashed { view: id });
    let wait = if crashes <= RELOADS { 1000 * crashes.pow(2) } else { u32::MAX };
    view.answered.set(std::time::Instant::now() + std::time::Duration::from_millis(u64::from(wait)));
    match crashes <= RELOADS {
        true => crate::glib::later(wait, move || load(id)),
        false => send(&FromWeb::Broken { view: id }),
    }
}

unsafe extern "C" fn loaded(web: Ptr, event: c_int, _data: Ptr) {
    if let Some(view) = by_web(web).filter(|_| event == 3) {
        view.loaded.set(true);
        send(&FromWeb::Loaded { view: view.id });
        let (id, crashes) = (view.id, view.crashes.get());
        if crashes > 0 {
            crate::glib::later(STABLE, move || {
                crate::state::view(id)
                    .filter(|v| v.crashes.get() == crashes)
                    .iter()
                    .for_each(|v| v.crashes.set(0));
            });
        }
    }
}
