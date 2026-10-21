//! Diagnostic consumer: connects to a stream by object serial, receives one
//! frame and prints its size and the color at the center.
//!
//! Usage: `probe <object-serial> [timeout-seconds]`

use pipewire as pw;
use pw::spa;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

#[derive(Default)]
struct Probe {
    format: spa::param::video::VideoInfoRaw,
    result: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let serial: u64 = args
        .next()
        .ok_or("usage: probe <object-serial> [timeout-seconds]")?
        .parse()?;
    let timeout: u64 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(5);

    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;
    let stream = pw::stream::StreamRc::new(
        core.clone(),
        "wprs-probe",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::TARGET_OBJECT => serial.to_string(),
        },
    )?;

    let probe = Rc::new(RefCell::new(Probe::default()));
    let ml = mainloop.downgrade();
    let p_param = probe.clone();
    let p_proc = probe.clone();
    let _listener = stream
        .add_local_listener::<()>()
        .param_changed(move |_, _, id, param| {
            if let Some(param) = param
                && id == spa::param::ParamType::Format.as_raw()
            {
                let _ = p_param.borrow_mut().format.parse(param);
            }
        })
        .process(move |stream, _| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let mut p = p_proc.borrow_mut();
            if p.result.is_some() {
                return;
            }
            let size = p.format.size();
            let format = p.format.format();
            let Some(data) = buffer.datas_mut().first_mut() else {
                return;
            };
            let stride = data.chunk().stride().max(0) as usize;
            let chunk_size = data.chunk().size() as usize;
            let Some(bytes) = data.data() else { return };
            if chunk_size == 0 || stride == 0 {
                return;
            }
            let (cx, cy) = (size.width as usize / 2, size.height as usize / 2);
            let i = cy * stride + cx * 4;
            let px = &bytes[i..i + 4];
            p.result = Some(format!(
                "frame {}x{} format {:?} stride {} center bgra {:?}",
                size.width, size.height, format, stride, px
            ));
            if let Some(ml) = ml.upgrade() {
                ml.quit();
            }
        })
        .register()?;

    let format = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(spa::pod::object!(
            spa::utils::SpaTypes::ObjectParamFormat,
            spa::param::ParamType::EnumFormat,
            spa::pod::property!(
                spa::param::format::FormatProperties::MediaType,
                Id,
                spa::param::format::MediaType::Video
            ),
            spa::pod::property!(
                spa::param::format::FormatProperties::MediaSubtype,
                Id,
                spa::param::format::MediaSubtype::Raw
            ),
            spa::pod::property!(
                spa::param::format::FormatProperties::VideoFormat,
                Choice,
                Enum,
                Id,
                spa::param::video::VideoFormat::BGRx,
                spa::param::video::VideoFormat::BGRx,
                spa::param::video::VideoFormat::BGRA
            ),
        )),
    )?
    .0
    .into_inner();
    let mut params = [spa::pod::Pod::from_bytes(&format).ok_or("bad pod")?];
    stream.connect(
        spa::utils::Direction::Input,
        None,
        pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;

    let ml = mainloop.downgrade();
    let timer = mainloop.loop_().add_timer(move |_| {
        if let Some(ml) = ml.upgrade() {
            ml.quit();
        }
    });
    timer
        .update_timer(Some(Duration::from_secs(timeout)), None)
        .into_result()?;
    mainloop.run();

    match probe.borrow().result.clone() {
        Some(r) => {
            println!("{r}");
            Ok(())
        }
        None => Err(format!("no frame received within {timeout}s").into()),
    }
}
