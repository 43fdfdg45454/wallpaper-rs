use pipewire::spa;
use spa::param::ParamType;
use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use spa::param::video::VideoFormat;
use spa::pod::deserialize::PodDeserializer;
use spa::pod::{ChoiceValue, Object, Pod, Property, PropertyFlags, Value, object, property};
use spa::utils::{Choice, ChoiceEnum, ChoiceFlags, Fraction, Rectangle, SpaTypes};

#[derive(Debug, PartialEq, Eq)]
pub enum Modifier {
    None,
    Fixed(u64),
    Choice(Vec<u64>),
}

fn serialize(object: Object) -> Vec<u8> {
    spa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(object))
        .expect("pod serialization into a Vec cannot fail")
        .0
        .into_inner()
}

fn int_property(key: u32, choice: ChoiceEnum<i32>) -> Property {
    Property::new(key, Value::Choice(ChoiceValue::Int(Choice(ChoiceFlags::empty(), choice))))
}

fn modifier_property(modifiers: &[u64], fixed: Option<u64>) -> Option<Property> {
    let key = FormatProperties::VideoModifier.as_raw();
    let alternatives: Vec<i64> = modifiers.iter().map(|&m| m as i64).collect();
    let (flags, value) = match (fixed, alternatives.first()) {
        (Some(m), _) => (PropertyFlags::MANDATORY, Value::Long(m as i64)),
        (None, Some(&default)) => {
            let choice = Choice(ChoiceFlags::empty(), ChoiceEnum::Enum { default, alternatives });
            (PropertyFlags::MANDATORY | PropertyFlags::DONT_FIXATE, Value::Choice(ChoiceValue::Long(choice)))
        }
        (None, None) => return None,
    };
    Some(Property { key, flags, value })
}

fn format(width: u32, height: u32, modifier: Option<Property>) -> Vec<u8> {
    let fps = |num| Fraction { num, denom: 1 };
    let (bgrx, bgra) = (VideoFormat::BGRx, VideoFormat::BGRA);
    let mut object = object!(
        SpaTypes::ObjectParamFormat,
        ParamType::EnumFormat,
        property!(FormatProperties::MediaType, Id, MediaType::Video),
        property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        property!(FormatProperties::VideoFormat, Choice, Enum, Id, bgrx, bgrx, bgra),
        property!(FormatProperties::VideoSize, Rectangle, Rectangle { width, height }),
        property!(FormatProperties::VideoFramerate, Fraction, fps(0)),
        property!(FormatProperties::VideoMaxFramerate, Choice, Range, Fraction, fps(60), fps(1), fps(240)),
    );
    object.properties.extend(modifier);
    serialize(object)
}

pub fn formats(width: u32, height: u32, modifiers: &[u64], fixed: Option<u64>) -> Vec<Vec<u8>> {
    let fixed = fixed.and_then(|m| modifier_property(&[], Some(m)));
    let offered = [fixed, modifier_property(modifiers, None)].into_iter().flatten();
    offered.map(Some).chain([None]).map(|modifier| format(width, height, modifier)).collect()
}

pub fn modifier(param: &Pod) -> Modifier {
    let Ok((_, Value::Object(object))) = PodDeserializer::deserialize_any_from(param.as_bytes()) else {
        return Modifier::None;
    };
    let key = FormatProperties::VideoModifier.as_raw();
    let Some(property) = object.properties.into_iter().find(|p| p.key == key) else { return Modifier::None };
    let open = property.flags.contains(PropertyFlags::DONT_FIXATE);
    match property.value {
        Value::Long(m) => Modifier::Fixed(m as u64),
        Value::Choice(ChoiceValue::Long(Choice(_, ChoiceEnum::Enum { default, alternatives }))) if open => {
            let mut all: Vec<u64> = std::iter::once(default).chain(alternatives).map(|m| m as u64).collect();
            all.dedup();
            Modifier::Choice(all)
        }
        Value::Choice(ChoiceValue::Long(Choice(
            _,
            ChoiceEnum::Enum { default, .. } | ChoiceEnum::None(default),
        ))) => Modifier::Fixed(default as u64),
        _ => Modifier::None,
    }
}

pub const META_TIMELINE: u32 = 9;
pub const DATA_SYNCOBJ: u32 = 5;
const BUFFERS_META_TYPE: u32 = 7;

fn buffer_object(width: u32, height: u32, dmabuf: bool) -> Object {
    let stride = (width * 4) as i32;
    let kind = 1 << if dmabuf { spa::sys::SPA_DATA_DmaBuf } else { spa::sys::SPA_DATA_MemFd };
    object!(
        SpaTypes::ObjectParamBuffers,
        ParamType::Buffers,
        int_property(spa::sys::SPA_PARAM_BUFFERS_buffers, ChoiceEnum::Range { default: 4, min: 4, max: 8 }),
        Property::new(spa::sys::SPA_PARAM_BUFFERS_blocks, Value::Int(1)),
        Property::new(spa::sys::SPA_PARAM_BUFFERS_size, Value::Int(stride * height as i32)),
        Property::new(spa::sys::SPA_PARAM_BUFFERS_stride, Value::Int(stride)),
        int_property(
            spa::sys::SPA_PARAM_BUFFERS_dataType,
            ChoiceEnum::Flags { default: kind, flags: vec![kind] }
        ),
    )
}

pub fn buffers(width: u32, height: u32, dmabuf: bool) -> Vec<u8> {
    serialize(buffer_object(width, height, dmabuf))
}

pub fn damage() -> Vec<u8> {
    let one = size_of::<spa::sys::spa_meta_region>() as i32;
    let most = one * crate::damage::REGIONS as i32;
    serialize(object!(
        SpaTypes::ObjectParamMeta,
        ParamType::Meta,
        Property::new(spa::sys::SPA_PARAM_META_type, Value::Id(spa::utils::Id(crate::damage::META_DAMAGE))),
        int_property(spa::sys::SPA_PARAM_META_size, ChoiceEnum::Range { default: most, min: one, max: most }),
    ))
}

pub fn synced(width: u32, height: u32) -> Vec<Vec<u8>> {
    let mut timed = buffer_object(width, height, true);
    let flags = |bits: i32| {
        Value::Choice(ChoiceValue::Int(Choice(
            ChoiceFlags::empty(),
            ChoiceEnum::Flags { default: bits, flags: vec![bits] },
        )))
    };
    let kinds = (1 << spa::sys::SPA_DATA_DmaBuf) | (1 << DATA_SYNCOBJ);
    for property in &mut timed.properties {
        match property.key {
            spa::sys::SPA_PARAM_BUFFERS_blocks => property.value = Value::Int(3),
            spa::sys::SPA_PARAM_BUFFERS_dataType => property.value = flags(kinds),
            _ => {}
        }
    }
    timed.properties.push(Property::new(BUFFERS_META_TYPE, flags(1 << META_TIMELINE)));
    let meta = object!(
        SpaTypes::ObjectParamMeta,
        ParamType::Meta,
        Property::new(spa::sys::SPA_PARAM_META_type, Value::Id(spa::utils::Id(META_TIMELINE))),
        Property::new(spa::sys::SPA_PARAM_META_size, Value::Int(24)),
    );
    vec![serialize(timed), serialize(meta), buffers(width, height, true)]
}

#[cfg(test)]
mod tests;
