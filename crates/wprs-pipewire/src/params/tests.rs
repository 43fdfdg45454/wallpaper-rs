use super::*;

fn object(bytes: &[u8]) -> Object {
    match PodDeserializer::deserialize_any_from(bytes) {
        Ok((_, Value::Object(object))) => object,
        _ => panic!("not an object pod"),
    }
}

fn pod(bytes: &[u8]) -> &Pod {
    Pod::from_bytes(bytes).expect("valid pod")
}

#[test]
fn pods_are_valid() {
    let all = [
        formats(1920, 1080, &[0, 7], Some(7)),
        vec![buffers(1920, 1080, false), buffers(8, 8, true), damage()],
    ];
    for bytes in all.iter().flatten() {
        assert!(Pod::from_bytes(bytes).is_some());
        assert_eq!(bytes.len() % 8, 0);
    }
}

#[test]
fn asks_for_at_least_four_buffers_of_the_negotiated_kind() {
    for (dmabuf, kind) in [(false, spa::sys::SPA_DATA_MemFd), (true, spa::sys::SPA_DATA_DmaBuf)] {
        let object = object(&buffers(64, 64, dmabuf));
        let value = |key| object.properties.iter().find(|p| p.key == key).map(|p| p.value.clone());
        let count = ChoiceEnum::Range { default: 4, min: 4, max: 8 };
        let kind = ChoiceEnum::Flags { default: 1 << kind, flags: vec![1 << kind] };
        let choice = |c| Some(Value::Choice(ChoiceValue::Int(Choice(ChoiceFlags::empty(), c))));
        assert_eq!(value(spa::sys::SPA_PARAM_BUFFERS_buffers), choice(count));
        assert_eq!(value(spa::sys::SPA_PARAM_BUFFERS_dataType), choice(kind));
    }
}

#[test]
fn damage_is_asked_for_as_an_array_of_up_to_sixteen_rectangles() {
    let object = object(&damage());
    let value = |key| object.properties.iter().find(|p| p.key == key).map(|p| p.value.clone());
    assert_eq!(value(spa::sys::SPA_PARAM_META_type), Some(Value::Id(spa::utils::Id(3))));
    let size = ChoiceEnum::Range { default: 256, min: 16, max: 256 };
    let choice = Value::Choice(ChoiceValue::Int(Choice(ChoiceFlags::empty(), size)));
    assert_eq!(value(spa::sys::SPA_PARAM_META_size), Some(choice));
}

#[test]
fn explicit_sync_is_offered_before_the_plain_gpu_buffers() {
    let [timed, meta, plain] = &synced(64, 64)[..] else { panic!("three parameters") };
    let value =
        |bytes: &[u8], key| object(bytes).properties.into_iter().find(|p| p.key == key).map(|p| p.value);
    assert_eq!(value(timed, spa::sys::SPA_PARAM_BUFFERS_blocks), Some(Value::Int(3)));
    assert!(value(timed, BUFFERS_META_TYPE).is_some() && value(plain, BUFFERS_META_TYPE).is_none());
    assert_eq!(value(plain, spa::sys::SPA_PARAM_BUFFERS_blocks), Some(Value::Int(1)));
    assert_eq!(value(meta, spa::sys::SPA_PARAM_META_type), Some(Value::Id(spa::utils::Id(META_TIMELINE))));
    assert!([timed, meta, plain].iter().all(|bytes| Pod::from_bytes(bytes).is_some()));
}

#[test]
fn offers_dma_bufs_first_and_shared_memory_last() {
    let [plain] = &formats(64, 64, &[], None)[..] else { panic!("one format") };
    assert_eq!(modifier(pod(plain)), Modifier::None);
    let [open, plain] = &formats(64, 64, &[0, 7], None)[..] else { panic!("two formats") };
    assert_eq!((modifier(pod(open)), modifier(pod(plain))), (Modifier::Choice(vec![0, 7]), Modifier::None));
    let [fixed, open, _] = &formats(64, 64, &[0, 7], Some(7))[..] else { panic!("three formats") };
    assert_eq!(
        (modifier(pod(fixed)), modifier(pod(open))),
        (Modifier::Fixed(7), Modifier::Choice(vec![0, 7]))
    );
    let flags = |bytes: &[u8]| object(bytes).properties.last().map(|p| p.flags);
    assert_eq!(flags(fixed), Some(PropertyFlags::MANDATORY));
    assert_eq!(flags(open), Some(PropertyFlags::MANDATORY | PropertyFlags::DONT_FIXATE));
}
