mod kpipewire;

use pipewire as pw;
use pw::spa;
use spa::param::ParamType;
use spa::param::video::{VideoFlags, VideoInfoRaw};
use spa::pod::Pod;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

type AnyError = Box<dyn std::error::Error>;

#[derive(Default)]
struct Probe {
    format: VideoInfoRaw,
    buffers: usize,
    frames: usize,
    skip: bool,
    result: Option<String>,
}

fn main() -> Result<(), AnyError> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: probe <object-serial | node:ID> [timeout-seconds] [--hold] [--late] [--dmabuf]";
    let target = args.next().ok_or(usage)?;
    let (serial, node_id) = match target.strip_prefix("node:") {
        Some(id) => (None, Some(id.parse::<u32>()?)),
        None => (Some(target.parse::<u64>()?), None),
    };
    let timeout: u64 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(5);
    let (options, known): (Vec<String>, _) = (args.collect(), ["--hold", "--late", "--dmabuf"]);
    if let Some(unknown) = options.iter().find(|o| !known.contains(&o.as_str())) {
        return Err(format!("unknown option {unknown}; {usage}").into());
    }
    let [hold, late, dmabuf] = known.map(|k| options.iter().any(|o| o == k));

    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;
    let mut props = pw::properties::properties! {
        *pw::keys::MEDIA_TYPE => "Video",
        *pw::keys::MEDIA_CATEGORY => "Capture",
    };
    if let Some(serial) = serial {
        props.insert(*pw::keys::TARGET_OBJECT, serial.to_string());
    }
    let stream = pw::stream::StreamRc::new(core.clone(), "wprs-probe", props)?;

    let probe = Rc::new(RefCell::new(Probe { skip: late, ..Probe::default() }));
    let (on_param, on_process, weak_loop) = (probe.clone(), probe.clone(), mainloop.downgrade());
    let (on_add, on_remove) = (probe.clone(), probe.clone());
    let _listener = stream
        .add_local_listener::<()>()
        .param_changed(move |stream, _, id, param| {
            if let Some(param) = param
                && id == ParamType::Format.as_raw()
            {
                let mut format = VideoInfoRaw::default();
                let _ = format.parse(param);
                on_param.borrow_mut().format = format;
                let bytes = kpipewire::buffers_param(format.flags().contains(VideoFlags::MODIFIER));
                if let Some(pod) = Pod::from_bytes(&bytes) {
                    let _ = stream.update_params(&mut [pod]);
                }
            }
        })
        .add_buffer(move |_, _, _| on_add.borrow_mut().buffers += 1)
        .remove_buffer(move |_, _, _| on_remove.borrow_mut().buffers -= 1)
        .process(move |stream, _| {
            let Some(mut buffer) = stream.dequeue_buffer() else { return };
            let mut probe = on_process.borrow_mut();
            let Some(data) = buffer.datas_mut().first_mut() else { return };
            let stride = data.chunk().stride().max(0) as usize;
            if data.chunk().size() == 0 || stride == 0 {
                return;
            }
            probe.frames += 1;
            if probe.result.is_some() {
                return;
            }
            if std::mem::take(&mut probe.skip) {
                return;
            }
            let result = kpipewire::describe(data, &probe.format, probe.buffers);
            let done = !hold || result.starts_with("error:");
            if !done {
                println!("{result}");
            }
            probe.result = Some(result);
            drop(probe);
            if done {
                quit(&weak_loop);
            }
        })
        .register()?;

    let formats = kpipewire::format_params(dmabuf);
    let mut params: Vec<&Pod> =
        formats.iter().map(|f| Pod::from_bytes(f).ok_or("bad pod")).collect::<Result<_, _>>()?;
    let flags = pw::stream::StreamFlags::AUTOCONNECT;
    stream.connect(spa::utils::Direction::Input, node_id, flags, &mut params)?;

    let timer_loop = mainloop.downgrade();
    let timer = mainloop.loop_().add_timer(move |_| quit(&timer_loop));
    timer.update_timer(Some(Duration::from_secs(timeout)), None).into_result()?;
    mainloop.run();

    let frames = probe.borrow().frames;
    let result = probe.borrow_mut().result.take();
    match result {
        Some(result) if result.starts_with("error:") => Err(result.into()),
        Some(result) => {
            match hold {
                true => println!("frames {frames}"),
                false => println!("{result}"),
            }
            Ok(())
        }
        None => Err(format!("no frame received within {timeout}s").into()),
    }
}

fn quit(mainloop: &pw::main_loop::MainLoopWeak) {
    if let Some(mainloop) = mainloop.upgrade() {
        mainloop.quit();
    }
}
