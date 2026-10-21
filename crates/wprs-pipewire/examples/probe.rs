//! Diagnostic consumer: connects to a stream by object serial, receives one
//! frame and prints its size and the color at the center.
//!
//! Usage: `probe <object-serial | node:ID> [timeout-seconds]`
//!
//! `node:ID` connects by node id, like older KPipeWire releases do. Buffers
//! are read the way KPipeWire reads them: it accepts MemFd or MemPtr, maps a
//! MemFd itself from offset 0 (ignoring `mapoffset`) and crashes on MemPtr,
//! so the probe fails unless every buffer is a MemFd readable that way.

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
    let target = args
        .next()
        .ok_or("usage: probe <object-serial | node:ID> [timeout-seconds]")?;
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
    let ml = mainloop.downgrade();
    let p_param = probe.clone();
    let p_proc = probe.clone();
    let _listener = stream
        .add_local_listener::<()>()
        .param_changed(move |stream, _, id, param| {
            if let Some(param) = param
                && id == spa::param::ParamType::Format.as_raw()
            {
                let _ = p_param.borrow_mut().format.parse(param);
                // Same buffer types as KPipeWire without DMA-BUF.
                let bytes = buffers_param();
                if let Some(pod) = spa::pod::Pod::from_bytes(&bytes) {
                    let _ = stream.update_params(&mut [pod]);
                }
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
            if chunk_size == 0 || stride == 0 {
                return;
            }
            let (cx, cy) = (size.width as usize / 2, size.height as usize / 2);
            let i = cy * stride + cx * 4;
            let raw = data.as_raw();
            p.result = Some(if data.type_() != spa::buffer::DataType::MemFd {
                format!("error: buffer type {:?}, KPipeWire needs MemFd", data.type_())
            } else {
                match read_like_kpipewire(raw.fd, raw.maxsize + raw.mapoffset, i) {
                    Ok(px) => format!(
                        "frame {}x{} format {:?} stride {} type MemFd center bgra {:?}",
                        size.width, size.height, format, stride, px
                    ),
                    Err(e) => format!("error: cannot map the MemFd buffer: {e}"),
                }
            });
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
        node_id,
        pw::stream::StreamFlags::AUTOCONNECT,
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
        Some(r) if r.starts_with("error:") => Err(r.into()),
        Some(r) => {
            println!("{r}");
            Ok(())
        }
        None => Err(format!("no frame received within {timeout}s").into()),
    }
}

/// Buffer types KPipeWire accepts when it does not use DMA-BUF.
fn buffers_param() -> Vec<u8> {
    let types = (1 << spa::sys::SPA_DATA_MemFd) | (1 << spa::sys::SPA_DATA_MemPtr);
    spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(spa::pod::Object {
            type_: spa::utils::SpaTypes::ObjectParamBuffers.as_raw(),
            id: spa::param::ParamType::Buffers.as_raw(),
            properties: vec![spa::pod::Property {
                key: spa::sys::SPA_PARAM_BUFFERS_dataType,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Choice(spa::pod::ChoiceValue::Int(spa::utils::Choice(
                    spa::utils::ChoiceFlags::empty(),
                    spa::utils::ChoiceEnum::Flags {
                        default: types,
                        flags: vec![types],
                    },
                ))),
            }],
        }),
    )
    .expect("pod serialization into a Vec cannot fail")
    .0
    .into_inner()
}

/// Maps the whole fd from offset 0 and reads 4 bytes at `offset`, ignoring
/// `mapoffset` like KPipeWire.
fn read_like_kpipewire(fd: i64, len: u32, offset: usize) -> Result<[u8; 4], Box<dyn std::error::Error>> {
    use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
    let fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(i32::try_from(fd)?) };
    let len = len as usize;
    if offset + 4 > len {
        return Err("pixel outside the mapping".into());
    }
    let map = unsafe {
        mmap(
            std::ptr::null_mut(),
            len,
            ProtFlags::READ,
            MapFlags::PRIVATE,
            fd,
            0,
        )?
    };
    let mut px = [0u8; 4];
    px.copy_from_slice(unsafe { std::slice::from_raw_parts(map.cast::<u8>().add(offset), 4) });
    unsafe { munmap(map, len)? };
    Ok(px)
}
