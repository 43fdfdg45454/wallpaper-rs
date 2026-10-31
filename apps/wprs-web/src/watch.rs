use crate::ffi::{Ptr, get};
use crate::state::View;
use std::rc::Rc;
use std::time::{Duration, Instant};
use wprs_web::FromWeb;

const BEAT: u32 = 5000;
const DEAF: Duration = Duration::from_secs(10);

pub fn beat(view: Rc<View>) {
    crate::glib::later(BEAT, move || tick(view));
}

fn tick(view: Rc<View>) {
    if crate::state::view(view.id).is_none_or(|v| !Rc::ptr_eq(&v, &view)) {
        return;
    }
    let (a, now) = (get(), Instant::now());
    let waiting = view.stopped.get() || view.answered.get() > now;
    match (waiting, now.duration_since(view.answered.get()) > DEAF) {
        (true, _) => {}
        (false, true) => {
            view.answered.set(now);
            crate::state::send(&FromWeb::Said("the page stopped answering and loads again".into()));
            unsafe { (a.webkit_web_view_terminate_web_process)(view.web) };
        }
        (false, false) => {
            let done = answered as unsafe extern "C" fn(Ptr, Ptr, Ptr) as Ptr;
            let (null, id) = (std::ptr::null(), view.id as usize as Ptr);
            unsafe {
                (a.webkit_web_view_evaluate_javascript)(
                    view.web,
                    c"0".as_ptr(),
                    -1,
                    null,
                    null,
                    std::ptr::null_mut(),
                    done,
                    id,
                )
            };
        }
    }
    beat(view);
}

unsafe extern "C" fn answered(web: Ptr, result: Ptr, data: Ptr) {
    let a = get();
    let mut error = std::ptr::null_mut();
    unsafe {
        let value = (a.webkit_web_view_evaluate_javascript_finish)(web, result, &mut error);
        if !value.is_null() {
            (a.g_object_unref)(value);
        }
        if !error.is_null() {
            (a.g_error_free)(error);
        }
    }
    if let Some(view) =
        crate::state::view(data as usize as u32).filter(|v| v.answered.get() <= Instant::now())
    {
        view.answered.set(Instant::now());
    }
}
