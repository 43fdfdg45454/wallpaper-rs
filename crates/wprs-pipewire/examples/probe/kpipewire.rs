use crate::AnyError;
use pipewire::spa;
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use spa::buffer::{Data, DataType};
use spa::param::ParamType;
use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use spa::param::video::{VideoFormat, VideoInfoRaw};
use spa::pod::{ChoiceValue, Object, Property, PropertyFlags, Value, object, property};
use spa::utils::{Choice, ChoiceEnum, ChoiceFlags, SpaTypes};
use std::os::fd::{AsRawFd, BorrowedFd};

const LINEAR: i64 = 0;
const SYNC: libc::c_ulong = 0x4008_6200;
const READ: u64 = 1;
const END: u64 = 4;

fn serialize(object: Object) -> Vec<u8> {
    spa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(object))
        .expect("pod serialization into a Vec cannot fail")
        .0
        .into_inner()
}

pub fn format_params(dmabuf: bool) -> Vec<Vec<u8>> {
    let (bgrx, bgra) = (VideoFormat::BGRx, VideoFormat::BGRA);
    let format = |modifier: Option<Property>| {
        let mut object = object!(
            SpaTypes::ObjectParamFormat,
            ParamType::EnumFormat,
            property!(FormatProperties::MediaType, Id, MediaType::Video),
            property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
            property!(FormatProperties::VideoFormat, Choice, Enum, Id, bgrx, bgrx, bgra),
        );
        object.properties.extend(modifier);
        serialize(object)
    };
    let linear =
        Choice(ChoiceFlags::empty(), ChoiceEnum::Enum { default: LINEAR, alternatives: vec![LINEAR] });
    let modifier = Property {
        key: FormatProperties::VideoModifier.as_raw(),
        flags: PropertyFlags::MANDATORY | PropertyFlags::DONT_FIXATE,
        value: Value::Choice(ChoiceValue::Long(linear)),
    };
    let offered = dmabuf.then_some(Some(modifier)).into_iter().chain([None]);
    offered.map(format).collect()
}

pub fn buffers_param(dmabuf: bool) -> Vec<u8> {
    let shared = (1 << spa::sys::SPA_DATA_MemFd) | (1 << spa::sys::SPA_DATA_MemPtr);
    let types = shared | if dmabuf { 1 << spa::sys::SPA_DATA_DmaBuf } else { 0 };
    let int_choice = |choice| Value::Choice(ChoiceValue::Int(Choice(ChoiceFlags::empty(), choice)));
    let count = int_choice(ChoiceEnum::Range { default: 3, min: 2, max: 16 });
    let data_type = int_choice(ChoiceEnum::Flags { default: types, flags: vec![types] });
    serialize(object!(
        SpaTypes::ObjectParamBuffers,
        ParamType::Buffers,
        Property::new(spa::sys::SPA_PARAM_BUFFERS_buffers, count),
        Property::new(spa::sys::SPA_PARAM_BUFFERS_align, Value::Int(16)),
        Property::new(spa::sys::SPA_PARAM_BUFFERS_dataType, data_type),
    ))
}

pub fn describe(data: &Data, info: &VideoInfoRaw, buffers: usize) -> String {
    let (size, raw, chunk) = (info.size(), data.as_raw(), data.chunk());
    let (stride, offset) = (chunk.stride().max(0) as usize, chunk.offset() as usize);
    let kind = match data.type_() {
        DataType::MemFd => "MemFd".to_owned(),
        DataType::DmaBuf => format!("DmaBuf modifier {:#x}", info.modifier()),
        other => return format!("error: buffer type {other:?}, KPipeWire needs MemFd or DmaBuf"),
    };
    let center = offset + size.height as usize / 2 * stride + size.width as usize / 2 * 4;
    let frame = offset..offset + size.height as usize * stride;
    let dmabuf = data.type_() == DataType::DmaBuf;
    match read_mapped(raw.fd, (raw.maxsize + raw.mapoffset) as usize, center, frame, dmabuf) {
        Ok((px, hash)) => format!(
            "frame {}x{} format {:?} stride {stride} type {kind} buffers {buffers} center bgra {px:?} hash {hash:016x}",
            size.width,
            size.height,
            info.format()
        ),
        Err(e) => format!("error: cannot map the {kind} buffer: {e}"),
    }
}

fn sync(fd: BorrowedFd<'_>, flags: u64) {
    let _ = unsafe { libc::ioctl(fd.as_raw_fd(), SYNC, &flags) };
}

fn read_mapped(
    fd: i64,
    len: usize,
    center: usize,
    frame: std::ops::Range<usize>,
    dmabuf: bool,
) -> Result<([u8; 4], u64), AnyError> {
    let fd = unsafe { BorrowedFd::borrow_raw(i32::try_from(fd)?) };
    if center + 4 > len {
        return Err("pixel outside the mapping".into());
    }
    let map = unsafe { mmap(std::ptr::null_mut(), len, ProtFlags::READ, MapFlags::SHARED, fd, 0)? };
    if dmabuf {
        sync(fd, READ);
    }
    let bytes = unsafe { std::slice::from_raw_parts(map.cast::<u8>(), len) };
    let px = [0, 1, 2, 3].map(|i| bytes[center + i]);
    let hash = bytes[frame.start.min(len)..frame.end.min(len)]
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3));
    if dmabuf {
        sync(fd, READ | END);
    }
    unsafe { munmap(map, len)? };
    Ok((px, hash))
}
