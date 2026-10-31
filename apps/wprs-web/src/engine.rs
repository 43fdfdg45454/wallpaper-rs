use crate::ffi::{Ptr, get, text};
use crate::glib::{NUL, connect, strings};
use crate::state::{Engine, FILTER, WAITING, engine, set_engine};
use std::path::PathBuf;
use wprs_web::{Link, Start, csp, filter};

thread_local! {
    pub static DISPLAY: std::cell::Cell<Ptr> = const { std::cell::Cell::new(std::ptr::null_mut()) };
    static STORE: std::cell::RefCell<PathBuf> = const { std::cell::RefCell::new(PathBuf::new()) };
}

pub fn start(link: Link, start: Start) -> Result<(), String> {
    crate::frames::patch()?;
    let (entries, preload) = crate::scheme::package(&start)?;
    let a = get();
    unsafe {
        let mut error = std::ptr::null_mut();
        let display = match &start.device {
            Some(node) => (a.wpe_display_headless_new_for_device)(text(node).as_ptr(), &mut error),
            None => (a.wpe_display_headless_new)(),
        };
        if display.is_null() || (a.wpe_display_connect)(display, &mut error) == 0 {
            return Err("the headless display of WPE WebKit cannot start".into());
        }
        DISPLAY.with(|d| d.set(display));
        crate::formats::offer(display, &start.modifiers)?;
        let memory = (a.webkit_memory_pressure_settings_new)();
        (a.webkit_memory_pressure_settings_set_memory_limit)(memory, 768);
        let kind = (a.webkit_web_context_get_type)();
        let context = (a.g_object_new)(kind, c"memory-pressure-settings".as_ptr(), memory, NUL);
        let (_owned, languages) = strings(&start.languages);
        (a.webkit_web_context_set_preferred_languages)(context, languages.as_ptr());
        let security = (a.webkit_web_context_get_security_manager)(context);
        for scheme in [c"wpak", c"wpak-user"] {
            let handler = crate::scheme::serve as unsafe extern "C" fn(Ptr, Ptr) as Ptr;
            (a.webkit_web_context_register_uri_scheme)(
                context,
                scheme.as_ptr(),
                handler,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            (a.webkit_security_manager_register_uri_scheme_as_secure)(security, scheme.as_ptr());
            (a.webkit_security_manager_register_uri_scheme_as_cors_enabled)(security, scheme.as_ptr());
        }
        let session = match &start.storage {
            Some(dir) => {
                let (data, cache) = (dir.join("data"), dir.join("cache"));
                let (data, cache) = (text(&data.to_string_lossy()), text(&cache.to_string_lossy()));
                (a.webkit_network_session_new)(data.as_ptr(), cache.as_ptr())
            }
            None => (a.webkit_network_session_new_ephemeral)(),
        };
        connect(session, "download-started", cancel as *const (), std::ptr::null_mut());
        let store = store().map_err(|e| format!("the network filter of the page cannot be built: {e}"))?;
        compile(&start.network, &store);
        STORE.with(|s| *s.borrow_mut() = store);
        set_engine(Engine { link, start, context, session, entries, preload });
    }
    Ok(())
}

unsafe extern "C" fn cancel(_session: Ptr, download: Ptr, _data: Ptr) {
    unsafe { (get().webkit_download_cancel)(download) };
}

fn store() -> std::io::Result<PathBuf> {
    use std::io::Read;
    use std::os::unix::fs::DirBuilderExt;
    let runtime =
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_absolute() && p.is_dir());
    let mut salt = [0u8; 8];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut salt)?;
    let name = format!("wprs-web-{}-{:016x}", std::process::id(), u64::from_le_bytes(salt));
    let dir = runtime.unwrap_or_else(std::env::temp_dir).join(name);
    std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
    Ok(dir)
}

unsafe fn compile(network: &[String], store: &std::path::Path) {
    let a = get();
    let rules = filter(network);
    unsafe {
        let path = text(&store.to_string_lossy());
        let store = (a.webkit_user_content_filter_store_new)(path.as_ptr());
        let bytes = (a.g_bytes_new)(rules.as_ptr().cast(), rules.len());
        let done = compiled as unsafe extern "C" fn(Ptr, Ptr, Ptr) as Ptr;
        (a.webkit_user_content_filter_store_save)(
            store,
            c"wprs".as_ptr(),
            bytes,
            std::ptr::null_mut(),
            done,
            std::ptr::null_mut(),
        );
        (a.g_bytes_unref)(bytes);
    }
}

unsafe extern "C" fn compiled(store: Ptr, result: Ptr, _data: Ptr) {
    let a = get();
    let mut error = std::ptr::null_mut();
    let made = unsafe { (a.webkit_user_content_filter_store_save_finish)(store, result, &mut error) };
    let _ = STORE.with(|s| std::fs::remove_dir_all(&*s.borrow()));
    if made.is_null() {
        crate::state::send(&wprs_web::FromWeb::Failed(
            "the network filter of the page cannot be built".into(),
        ));
        std::process::exit(1);
    }
    FILTER.with(|f| f.set(made));
    let waiting = WAITING.with(|w| std::mem::take(&mut *w.borrow_mut()));
    waiting.into_iter().for_each(crate::run::handle);
}

pub fn policy() -> String {
    engine().map_or_else(String::new, |e| csp(&e.start.network))
}

pub fn uri(entry: &str) -> String {
    let id = engine().map_or("", |e| e.start.id.as_str());
    format!("wpak://{id}/{entry}")
}
