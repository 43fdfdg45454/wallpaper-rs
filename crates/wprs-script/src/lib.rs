mod clock;
mod frame;
mod host;
mod sandbox;

pub use frame::Frame;
pub use host::{Host, serve};
pub use sandbox::enter as sandbox;

use clock::{Clock, cpu};
use rquickjs::loader::{BuiltinLoader, BuiltinResolver};
use rquickjs::{Array, Context, Ctx, Function, Module, Object, Persistent, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::rc::Rc;
use std::time::Duration;
use wprs_scene::{AudioFrame, PointerState, ScriptOutputs, Values};

const PRELUDE: &str = include_str!("prelude.js");
const MAX_FAILURES: u32 = 3;
const MAX_OUTPUT: usize = 64 << 10;
const RANDOM: &str = "((s) => () => ((s ^= s << 13, s ^= s >>> 17, s ^= s << 5) >>> 0) / 4294967296)";

pub type Source = (String, String);

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

type Update = (String, Persistent<Function<'static>>);

pub struct Script {
    updates: Vec<Update>,
    state: Persistent<Object<'static>>,
    context: Context,
    clock: Clock,
    limits: Limits,
    failures: u32,
}

impl Script {
    pub fn load(
        sources: &[Source],
        seed: u64,
        (locale, labels): (&str, &Json),
        limits: Limits,
    ) -> Result<Script, Error> {
        let failed = |e: rquickjs::Error| Error::Script(e.to_string());
        let runtime = Runtime::new().map_err(failed)?;
        runtime.set_memory_limit(limits.memory);
        runtime.set_max_stack_size(limits.stack);
        let resolver = BuiltinResolver::default().with_module("wprs");
        runtime.set_loader(resolver, BuiltinLoader::default().with_module("wprs", PRELUDE));
        let clock = Clock { deadline: Rc::default(), fired: Rc::default() };
        let (deadline, fired) = (clock.deadline.clone(), clock.fired.clone());
        runtime.set_interrupt_handler(Some(Box::new(move || {
            fired.set(fired.get() || deadline.get().is_some_and(|at| cpu() > at));
            fired.get()
        })));
        let context = Context::full(&runtime).map_err(failed)?;
        let empty = (Values::new(), AudioFrame::default(), PointerState::default());
        let frame = Frame {
            time: 0.0,
            dt: 0.0,
            props: &empty.0,
            audio: &empty.1,
            pointer: &empty.2,
            actions: &[],
            wall: 0.0,
            zone: "",
            timelines: &[],
            fired: &[],
        };
        let (frame, labels) = (frame.json(), labels.to_string());
        let random = format!("Math.random = {RANDOM}({})", seed as u32 | 1);
        let (state, updates) = clock.run(&context, limits.load, |ctx| {
            let state = Object::new(ctx.clone())?;
            state.set("frame", ctx.json_parse(frame)?)?;
            state.set("locale", locale)?;
            state.set("labels", ctx.json_parse(labels)?)?;
            state.set("out", outputs(ctx)?)?;
            let format = |p: String, s: f64, l: String| wprs_scene::format_time(&p, s, &l, true).0;
            state.set("format", Function::new(ctx.clone(), format)?)?;
            ctx.globals().set("__wprs", state.clone())?;
            ctx.eval::<(), _>(random)?;
            Module::import(ctx, "wprs")?.finish::<()>()?;
            let mut updates = Vec::new();
            for (path, source) in sources {
                let (module, done) = Module::declare(ctx.clone(), path.as_str(), source.as_str())?.eval()?;
                done.finish::<()>()?;
                let update: Option<Function> = module.get("update")?;
                updates.extend(update.map(|f| (path.clone(), Persistent::save(ctx, f))));
                if let Some(heard) = module.get::<_, Option<Function>>("timelineEvent")? {
                    let listeners: Array = state.get("listeners")?;
                    listeners.set(listeners.len(), heard)?;
                }
            }
            Ok((Persistent::save(ctx, state), updates))
        })?;
        Ok(Script { updates, state, context, clock, limits, failures: 0 })
    }

    pub fn update(&mut self, frame: &Frame) -> Result<ScriptOutputs, Error> {
        self.update_json(&frame.json())
    }

    pub fn update_json(&mut self, frame: &str) -> Result<ScriptOutputs, Error> {
        if self.failures >= MAX_FAILURES {
            return Err(Error::Disabled);
        }
        let (state, updates) = (self.state.clone(), self.updates.clone());
        let text = self.clock.run(&self.context, self.limits.update, |ctx| {
            let (state, frame, out) = (state.restore(ctx)?, ctx.json_parse(frame)?, outputs(ctx)?);
            state.set("out", out.clone())?;
            state.set("frame", frame.clone())?;
            state.get::<_, Function>("dispatch")?.call::<_, ()>((frame.clone(),))?;
            let values = Object::new(ctx.clone())?;
            for (path, update) in updates {
                let value: rquickjs::Value = update.restore(ctx)?.call((frame.clone(),))?;
                if !value.is_undefined() {
                    values.set(path, value)?;
                }
            }
            out.set("values", values)?;
            ctx.json_stringify(out)?.map(|s| s.to_string()).transpose().map(Option::unwrap_or_default)
        });
        let outputs = text.and_then(|t| match t.len() > MAX_OUTPUT {
            true => Err(Error::Script(format!("the script outputs take more than {MAX_OUTPUT} bytes"))),
            false => serde_json::from_str(&t).map_err(|e| Error::Script(format!("invalid value: {e}"))),
        });
        self.failures = if outputs.is_ok() { 0 } else { self.failures + 1 };
        outputs
    }
}

fn outputs<'js>(ctx: &Ctx<'js>) -> rquickjs::Result<Object<'js>> {
    let out = Object::new(ctx.clone())?;
    out.set("layers", Object::new(ctx.clone())?)?;
    out.set("timelines", Array::new(ctx.clone())?)?;
    Ok(out)
}
