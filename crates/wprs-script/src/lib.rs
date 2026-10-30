mod apply;
mod clock;
mod frame;
mod host;
mod modules;
mod sandbox;
mod setup;
mod wire;

pub use frame::{Encoder, Frame, Mode, Touch};
pub use host::{Host, serve};
pub use sandbox::enter as sandbox;
pub use setup::{Setup, Source};

use clock::{Clock, cpu};
use modules::Modules;
use rquickjs::{Array, CatchResultExt, Context, Function, Module, Object, Persistent, Runtime};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::time::Duration;
use wprs_scene::ScriptOutputs;

const MAX_FAILURES: u32 = 3;
const MAX_OUTPUT: usize = 256 << 10;
const RANDOM: &str = "((s) => () => ((s ^= s << 13, s ^= s >>> 17, s ^= s << 5) >>> 0) / 4294967296)";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub memory: usize,
    pub stack: usize,
    pub update: Duration,
    pub load: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        let (update, load) = (Duration::from_millis(4), Duration::from_millis(100));
        Limits { memory: 32 << 20, stack: 256 << 10, update, load }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
pub enum Error {
    #[error("the script used more than {0:?} of CPU time")]
    Timeout(Duration),
    #[error("the script used more than its memory limit")]
    Memory,
    #[error("script error: {0}")]
    Script(String),
    #[error("the script host failed: {0}")]
    Host(String),
    #[error("scripts are disabled after {MAX_FAILURES} failures in a row")]
    Disabled,
}

pub struct Script {
    state: Persistent<Object<'static>>,
    context: Context,
    clock: Clock,
    limits: Limits,
    failures: u32,
    started: bool,
    last: String,
    encoder: Encoder,
    zone: bool,
}

impl Script {
    pub fn load(setup: &Setup, limits: Limits) -> Result<Script, Error> {
        setup.fits().map_err(Error::Script)?;
        let failed = |e: rquickjs::Error| Error::Script(e.to_string());
        let runtime = Runtime::new().map_err(failed)?;
        runtime.set_memory_limit(limits.memory);
        runtime.set_max_stack_size(limits.stack);
        let modules = Modules(setup.sources.iter().cloned().collect());
        runtime.set_loader(modules.clone(), modules);
        let clock = Clock { deadline: Rc::default(), fired: Rc::default() };
        let (deadline, fired) = (clock.deadline.clone(), clock.fired.clone());
        runtime.set_interrupt_handler(Some(Box::new(move || {
            fired.set(fired.get() || deadline.get().is_some_and(|at| cpu() > at));
            fired.get()
        })));
        let context = Context::full(&runtime).map_err(failed)?;
        let mut shown = serde_json::to_value(setup).unwrap_or_default();
        shown.as_object_mut().map(|o| o.remove("sources"));
        let (shown, late) = (shown.to_string(), clock.fired.clone());
        let random = format!("Math.random = {RANDOM}({})", setup.seed as u32 | 1);
        let state = clock.run(&context, limits.load, |ctx| {
            let state = Object::new(ctx.clone())?;
            state.set("setup", ctx.json_parse(shown)?)?;
            let format = |p: String, s: f64, l: String| wprs_scene::format_time(&p, s, &l, true).0;
            state.set("format", Function::new(ctx.clone(), format)?)?;
            ctx.globals().set("__wprs", state.clone())?;
            ctx.eval::<(), _>(random)?;
            Module::import(ctx, "wprs")?.finish::<()>()?;
            let roots = Array::new(ctx.clone())?;
            for (i, (path, _)) in setup.sources.iter().enumerate() {
                let entry = Array::new(ctx.clone())?;
                entry.set(0, path.as_str())?;
                state.set("loading", path.as_str())?;
                match Module::import(ctx, path.as_str()).and_then(|p| p.finish::<Object>()).catch(ctx) {
                    Ok(names) => entry.set(1, names)?,
                    Err(_) if late.get() => return Err(rquickjs::Error::Unknown),
                    Err(e) => entry.set(2, e.to_string().split_whitespace().collect::<Vec<_>>().join(" "))?,
                }
                roots.set(i, entry)?;
            }
            state.get::<_, Function>("start")?.call::<_, ()>((roots,))?;
            Ok(Persistent::save(ctx, state))
        })?;
        let (last, encoder) = (String::new(), Encoder::default());
        Ok(Script { state, context, clock, limits, failures: 0, started: false, last, encoder, zone: false })
    }

    pub(crate) fn following_zone(self) -> Script {
        Script { zone: true, ..self }
    }

    pub fn run(&mut self, frame: &[u8]) -> Result<Option<String>, Error> {
        if self.failures >= MAX_FAILURES {
            return Err(Error::Disabled);
        }
        let (state, zone) = (self.state.clone(), self.zone);
        let budget = if self.started { self.limits.update } else { self.limits.load };
        let text = self.clock.run(&self.context, budget, |ctx| {
            let state = state.restore(ctx)?;
            if let Some(rule) = apply::apply(ctx, &state, frame)?.filter(|z| zone && !z.is_empty()) {
                host::set_zone(&rule);
            }
            let out: Object = state.get::<_, Function>("tick")?.call(())?;
            let text = ctx.json_stringify(out)?.map(|s| s.to_string()).transpose()?.unwrap_or_default();
            Ok((text, state.get::<_, bool>("once")?))
        });
        self.started = true;
        let text = text.and_then(|(t, once)| match t.len() > MAX_OUTPUT {
            true => Err(Error::Script(format!("the script outputs take more than {} KiB", MAX_OUTPUT >> 10))),
            false => Ok((t, once)),
        });
        self.failures = if text.is_ok() { 0 } else { self.failures + 1 };
        let (text, once) = text?;
        if text == self.last && !once {
            return Ok(None);
        }
        self.last = text.clone();
        Ok(Some(text))
    }

    pub fn update(&mut self, frame: &Frame) -> Result<ScriptOutputs, Error> {
        let bytes = self.encoder.encode(frame).to_vec();
        self.run(&bytes)?;
        serde_json::from_str(&self.last).map_err(|e| Error::Script(format!("invalid value: {e}")))
    }

    pub fn destroy(&mut self) {
        let state = self.state.clone();
        let _ = self.clock.run(&self.context, self.limits.load, |ctx| {
            state.restore(ctx)?.get::<_, Function>("destroy")?.call::<_, ()>(())
        });
    }
}
