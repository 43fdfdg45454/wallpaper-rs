use pipewire::spa;
use spa::param::ParamType;
use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use spa::param::video::VideoFormat;
use spa::pod::{ChoiceValue, Object, Property, Value, object, property};
use spa::utils::{Choice, ChoiceEnum, ChoiceFlags, Fraction, Rectangle, SpaTypes};

fn serialize(object: Object) -> Vec<u8> {
    spa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(object))
        .expect("pod serialization into a Vec cannot fail")
        .0
        .into_inner()
}

fn int_property(key: u32, choice: ChoiceEnum<i32>) -> Property {
    Property::new(key, Value::Choice(ChoiceValue::Int(Choice(ChoiceFlags::empty(), choice))))
}

pub fn enum_format(width: u32, height: u32) -> Vec<u8> {
    let fps = |num| Fraction { num, denom: 1 };
    let (bgrx, bgra) = (VideoFormat::BGRx, VideoFormat::BGRA);
    serialize(object!(
        SpaTypes::ObjectParamFormat,
        ParamType::EnumFormat,
        property!(FormatProperties::MediaType, Id, MediaType::Video),
        property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        property!(FormatProperties::VideoFormat, Choice, Enum, Id, bgrx, bgrx, bgra),
        property!(FormatProperties::VideoSize, Rectangle, Rectangle { width, height }),
        property!(FormatProperties::VideoFramerate, Fraction, fps(0)),
        property!(FormatProperties::VideoMaxFramerate, Choice, Range, Fraction, fps(60), fps(1), fps(240)),
    ))
}

pub fn buffers(width: u32, height: u32) -> Vec<u8> {
    let stride = (width * 4) as i32;
    let memfd_only = 1 << spa::sys::SPA_DATA_MemFd;
    serialize(object!(
        SpaTypes::ObjectParamBuffers,
        ParamType::Buffers,
        int_property(spa::sys::SPA_PARAM_BUFFERS_buffers, ChoiceEnum::Range { default: 4, min: 4, max: 8 }),
        Property::new(spa::sys::SPA_PARAM_BUFFERS_blocks, Value::Int(1)),
        Property::new(spa::sys::SPA_PARAM_BUFFERS_size, Value::Int(stride * height as i32)),
        Property::new(spa::sys::SPA_PARAM_BUFFERS_stride, Value::Int(stride)),
        int_property(
            spa::sys::SPA_PARAM_BUFFERS_dataType,
            ChoiceEnum::Flags { default: memfd_only, flags: vec![memfd_only] }
        ),
    ))
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

    #[test]
    fn asks_for_at_least_four_buffers() {
        let bytes = buffers(64, 64);
        let Ok((_, Value::Object(object))) =
            spa::pod::deserialize::PodDeserializer::deserialize_any_from(&bytes)
        else {
            panic!("buffers pod is not an object")
        };
        let count = object.properties.iter().find(|p| p.key == spa::sys::SPA_PARAM_BUFFERS_buffers);
        let range = count.map(|p| &p.value);
        let expected = ChoiceEnum::Range { default: 4, min: 4, max: 8 };
        assert!(matches!(range, Some(Value::Choice(ChoiceValue::Int(Choice(_, c)))) if *c == expected));
    }
}
