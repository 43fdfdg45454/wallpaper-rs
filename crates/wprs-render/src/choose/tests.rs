use super::listing::extra;
use super::{Fallback, kind, pick, settle};
use crate::RenderError;
use wgpu::{Backend, Backends, DeviceType};
use wprs_core::GpuChoice;

fn card(name: &str, t: DeviceType, b: Backend) -> (String, DeviceType, Backend) {
    (name.into(), t, b)
}

fn desk() -> Vec<(String, DeviceType, Backend)> {
    vec![
        card("Example Integrated Graphics (RADV)", DeviceType::IntegratedGpu, Backend::Gl),
        card("Example Integrated Graphics (RADV)", DeviceType::IntegratedGpu, Backend::Vulkan),
        card("NVIDIA GeForce RTX 5070 Ti", DeviceType::DiscreteGpu, Backend::Vulkan),
        card("NVIDIA GeForce RTX 4060", DeviceType::DiscreteGpu, Backend::Vulkan),
        card("Virtio-GPU Venus", DeviceType::VirtualGpu, Backend::Vulkan),
        card("llvmpipe (LLVM 19.1.7, 256 bits)", DeviceType::Cpu, Backend::Gl),
        card("llvmpipe (LLVM 19.1.7, 256 bits)", DeviceType::Cpu, Backend::Vulkan),
        card("Unknown", DeviceType::Other, Backend::Vulkan),
    ]
}

#[test]
fn a_graphics_card_is_chosen_by_kind_or_by_part_of_its_name() {
    let all = desk();
    let named = |part: &str| pick(&GpuChoice::Named(part.into()), &all);
    assert_eq!(pick(&GpuChoice::Auto, &all), None, "the automatic choice is left to the old path");
    assert_eq!(pick(&GpuChoice::Integrated, &all), Some(1), "Vulkan comes before OpenGL ES");
    assert_eq!(pick(&GpuChoice::Dedicated, &all), Some(2), "the first of two dedicated cards");
    assert_eq!(pick(&GpuChoice::Software, &all), Some(6));
    assert_eq!(
        [named("4060"), named("geforce"), named("RTX 5070 TI"), named("radv")],
        [3, 2, 2, 1].map(Some)
    );
    assert_eq!([named("venus"), named("Unknown")], [Some(4), Some(7)]);
    assert_eq!(named("Arc A770"), None, "a missing name finds nothing");
    let laptop = vec![all[0].clone(), all[5].clone()];
    assert_eq!(pick(&GpuChoice::Software, &laptop), Some(1), "OpenGL ES when Vulkan has none");
    assert_eq!(
        [pick(&GpuChoice::Dedicated, &laptop), pick(&GpuChoice::Integrated, &laptop)],
        [None, Some(0)]
    );
    assert_eq!(pick(&GpuChoice::Dedicated, &[]), None);
    let kinds = all.iter().map(|(_, t, _)| kind(*t)).collect::<Vec<_>>();
    assert_eq!(
        kinds,
        ["integrated", "integrated", "dedicated", "dedicated", "virtual", "software", "software", "other"]
    );
}

#[test]
fn a_chosen_card_that_cannot_open_falls_back_to_the_automatic_choice() {
    let failed = Some(Err::<u8, _>(RenderError::Device("no device for these limits".into())));
    assert_eq!(settle(failed, "dedicated"), Err(Fallback::Unusable));
    assert_eq!(settle(None::<Result<u8, RenderError>>, "Arc A770"), Err(Fallback::Missing));
    assert_eq!(settle(Some(Ok(7)), "dedicated"), Ok(7));
}

#[test]
fn a_backend_limit_keeps_other_drivers_unloaded_when_listing_cards() {
    assert_eq!(extra(Backend::Gl, Some(Backends::GL)), None, "WGPU_BACKEND=gl never loads Vulkan");
    assert_eq!(extra(Backend::Gl, None), Some(Backends::VULKAN));
    assert_eq!(extra(Backend::Vulkan, None), None);
    assert_eq!(extra(Backend::Vulkan, Some(Backends::VULKAN)), None);
}
