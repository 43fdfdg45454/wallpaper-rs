//! SPA parameter pods for the video streams.

use pipewire::spa;
use spa::param::ParamType;
use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use spa::param::video::VideoFormat;
use spa::pod::{ChoiceValue, Object, Property, PropertyFlags, Value};
use spa::utils::{Choice, ChoiceEnum, ChoiceFlags, Fraction, Id, Rectangle, SpaTypes};

fn serialize(object: Object) -> Vec<u8> {
    spa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(object))
        .expect("pod serialization into a Vec cannot fail")
        .0
        .into_inner()
}

fn prop(key: u32, value: Value) -> Property {
    Property {
        key,
        flags: PropertyFlags::empty(),
        value,
    }
}

/// Fixed-size raw video in BGRx (preferred) or BGRA, variable frame rate.
pub fn enum_format(width: u32, height: u32) -> Vec<u8> {
    serialize(Object {
        type_: SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: vec![
            prop(
                FormatProperties::MediaType.as_raw(),
                Value::Id(Id(MediaType::Video.as_raw())),
            ),
            prop(
                FormatProperties::MediaSubtype.as_raw(),
                Value::Id(Id(MediaSubtype::Raw.as_raw())),
            ),
            prop(
                FormatProperties::VideoFormat.as_raw(),
                Value::Choice(ChoiceValue::Id(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Enum {
                        default: Id(VideoFormat::BGRx.as_raw()),
                        alternatives: vec![Id(VideoFormat::BGRx.as_raw()), Id(VideoFormat::BGRA.as_raw())],
                    },
                ))),
            ),
            prop(
                FormatProperties::VideoSize.as_raw(),
                Value::Rectangle(Rectangle { width, height }),
            ),
            prop(
                FormatProperties::VideoFramerate.as_raw(),
                Value::Fraction(Fraction { num: 0, denom: 1 }),
            ),
            prop(
                FormatProperties::VideoMaxFramerate.as_raw(),
                Value::Choice(ChoiceValue::Fraction(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Range {
                        default: Fraction { num: 60, denom: 1 },
                        min: Fraction { num: 1, denom: 1 },
                        max: Fraction { num: 240, denom: 1 },
                    },
                ))),
            ),
        ],
    })
}

/// Buffer requirements: tightly packed rows, memfd or plain memory.
pub fn buffers(width: u32, height: u32) -> Vec<u8> {
    let stride = (width * 4) as i32;
    let size = stride * height as i32;
    let data_types = (1 << spa::sys::SPA_DATA_MemFd) | (1 << spa::sys::SPA_DATA_MemPtr);
    serialize(Object {
        type_: SpaTypes::ObjectParamBuffers.as_raw(),
        id: ParamType::Buffers.as_raw(),
        properties: vec![
            prop(
                spa::sys::SPA_PARAM_BUFFERS_buffers,
                Value::Choice(ChoiceValue::Int(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Range {
                        default: 3,
                        min: 2,
                        max: 8,
                    },
                ))),
            ),
            prop(spa::sys::SPA_PARAM_BUFFERS_blocks, Value::Int(1)),
            prop(spa::sys::SPA_PARAM_BUFFERS_size, Value::Int(size)),
            prop(spa::sys::SPA_PARAM_BUFFERS_stride, Value::Int(stride)),
            prop(
                spa::sys::SPA_PARAM_BUFFERS_dataType,
                Value::Choice(ChoiceValue::Int(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Flags {
                        default: data_types,
                        flags: vec![data_types],
                    },
                ))),
            ),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pods_are_valid() {
        for bytes in [enum_format(1920, 1080), buffers(1920, 1080)] {
            assert!(spa::pod::Pod::from_bytes(&bytes).is_some());
            assert_eq!(bytes.len() % 8, 0);
        }
    }
}
