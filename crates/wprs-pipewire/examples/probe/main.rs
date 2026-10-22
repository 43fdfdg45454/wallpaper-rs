mod kpipewire;

use pipewire as pw;
use pw::spa;
use spa::param::ParamType;
use spa::param::video::VideoInfoRaw;
use spa::pod::Pod;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

type AnyError = Box<dyn std::error::Error>;

#[derive(Default)]
struct Probe {
    format: VideoInfoRaw,
    buffers: usize,
    result: Option<String>,
}

fn main() -> Result<(), AnyError> {
    let mut args = std::env::args().skip(1);
    let target = args.next().ok_or("usage: probe <object-serial | node:ID> [timeout-seconds]")?;
    let (serial, node_id) = match target.strip_prefix("node:") {
        Some(id) => (None, Some(id.parse::<u32>()?)),
        None => (Some(target.parse::<u64>()?), None),
    };
    let timeout: u64 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(5);

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

    let probe = Rc::new(RefCell::new(Probe::default()));
    let (on_param, on_process, weak_loop) = (probe.clone(), probe.clone(), mainloop.downgrade());
    let (on_add, on_remove) = (probe.clone(), probe.clone());
    let _listener = stream
        .add_local_listener::<()>()
        .param_changed(move |stream, _, id, param| {
            if let Some(param) = param
                && id == ParamType::Format.as_raw()
            {
                let _ = on_param.borrow_mut().format.parse(param);
                let bytes = kpipewire::buffers_param();
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
            if probe.result.is_some() || data.chunk().size() == 0 || stride == 0 {
                return;
            }
            probe.result = Some(kpipewire::describe(data, &probe.format, stride, probe.buffers));
            quit(&weak_loop);
        })
        .register()?;

    let format = kpipewire::format_param();
    let mut params = [Pod::from_bytes(&format).ok_or("bad pod")?];
    let flags = pw::stream::StreamFlags::AUTOCONNECT;
    stream.connect(spa::utils::Direction::Input, node_id, flags, &mut params)?;

    let timer_loop = mainloop.downgrade();
    let timer = mainloop.loop_().add_timer(move |_| quit(&timer_loop));
    timer.update_timer(Some(Duration::from_secs(timeout)), None).into_result()?;
    mainloop.run();

    match probe.borrow_mut().result.take() {
        Some(result) if result.starts_with("error:") => Err(result.into()),
        Some(result) => {
            println!("{result}");
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
