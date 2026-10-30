use crate::Error;
use rquickjs::{CatchResultExt, Context, Ctx};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

pub(crate) struct Clock {
    pub(crate) deadline: Rc<Cell<Option<Duration>>>,
    pub(crate) fired: Rc<Cell<bool>>,
}

impl Clock {
    pub(crate) fn run<T>(
        &self,
        context: &Context,
        budget: Duration,
        work: impl FnOnce(&Ctx) -> rquickjs::Result<T>,
    ) -> Result<T, Error> {
        self.fired.set(false);
        self.deadline.set(Some(cpu() + budget));
        let result = context.with(|ctx| work(&ctx).catch(&ctx).map_err(|e| e.to_string()));
        let late = self.deadline.take().is_some_and(|at| cpu() > at);
        let result = result.map_err(|text| match self.fired.get() {
            true => Error::Timeout(budget),
            false if text.contains("out of memory") => Error::Memory,
            false => Error::Script(text.chars().take(512).collect()),
        });
        result.and_then(|value| if late { Err(Error::Timeout(budget)) } else { Ok(value) })
    }
}

pub(crate) fn cpu() -> Duration {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    Duration::new(t.tv_sec as u64, t.tv_nsec as u32)
}

unsafe extern "C" {
    fn tzset();
}

pub(crate) fn set_zone(rule: &str) {
    unsafe { std::env::set_var("TZ", rule) };
    unsafe { tzset() };
}
