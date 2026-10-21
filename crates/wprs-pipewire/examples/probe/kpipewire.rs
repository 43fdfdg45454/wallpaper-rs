use crate::AnyError;
use pipewire::spa;
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use spa::buffer::{Data, DataType};
use spa::param::ParamType;
use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use spa::param::video::{VideoFormat, VideoInfoRaw};
use spa::pod::{ChoiceValue, Object, Property, Value, object, property};
use spa::utils::{Choice, ChoiceEnum, ChoiceFlags, SpaTypes};
use std::os::fd::BorrowedFd;

fn serialize(object: Object) -> Vec<u8> {
    spa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(object))
        .expect("pod serialization into a Vec cannot fail")
        .0
        .into_inner()
}

pub fn format_param() -> Vec<u8> {
    let (bgrx, bgra) = (VideoFormat::BGRx, VideoFormat::BGRA);
    serialize(object!(
        SpaTypes::ObjectParamFormat,
        ParamType::EnumFormat,
        property!(FormatProperties::MediaType, Id, MediaType::Video),
        property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        property!(FormatProperties::VideoFormat, Choice, Enum, Id, bgrx, bgrx, bgra),
    ))
}

pub fn buffers_param() -> Vec<u8> {
    let types = (1 << spa::sys::SPA_DATA_MemFd) | (1 << spa::sys::SPA_DATA_MemPtr);
    let choice = Choice(ChoiceFlags::empty(), ChoiceEnum::Flags { default: types, flags: vec![types] });
    let data_type = Value::Choice(ChoiceValue::Int(choice));
    let property = Property::new(spa::sys::SPA_PARAM_BUFFERS_dataType, data_type);
    serialize(object!(SpaTypes::ObjectParamBuffers, ParamType::Buffers, property))
}

pub fn describe(data: &Data, info: &VideoInfoRaw, stride: usize) -> String {
    let (size, raw) = (info.size(), data.as_raw());
    let center = size.height as usize / 2 * stride + size.width as usize / 2 * 4;
    if data.type_() != DataType::MemFd {
        return format!("error: buffer type {:?}, KPipeWire needs MemFd", data.type_());
    }
    match read_mapped_from_offset_zero(raw.fd, raw.maxsize + raw.mapoffset, center) {
        Ok(px) => format!(
            "frame {}x{} format {:?} stride {stride} type MemFd center bgra {px:?}",
            size.width,
            size.height,
            info.format()
        ),
        Err(e) => format!("error: cannot map the MemFd buffer: {e}"),
    }
}

fn read_mapped_from_offset_zero(fd: i64, len: u32, offset: usize) -> Result<[u8; 4], AnyError> {
    let fd = unsafe { BorrowedFd::borrow_raw(i32::try_from(fd)?) };
    let len = len as usize;
    if offset + 4 > len {
        return Err("pixel outside the mapping".into());
    }
    let map = unsafe { mmap(std::ptr::null_mut(), len, ProtFlags::READ, MapFlags::PRIVATE, fd, 0)? };
    let px = unsafe { map.cast::<u8>().add(offset).cast::<[u8; 4]>().read() };
    unsafe { munmap(map, len)? };
    Ok(px)
}
