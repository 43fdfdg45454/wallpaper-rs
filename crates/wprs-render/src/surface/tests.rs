use super::*;
use wgpu::TextureFormat::*;

#[test]
fn desktop_presentation_never_waits_for_vsync_when_it_can_avoid_it() {
    assert_eq!(present_mode(&[PresentMode::Fifo, PresentMode::Mailbox], false), PresentMode::Mailbox);
    assert_eq!(present_mode(&[PresentMode::Immediate, PresentMode::Fifo], false), PresentMode::Immediate);
    assert_eq!(present_mode(&[PresentMode::Fifo], false), PresentMode::Fifo);
    assert_eq!(present_mode(&[PresentMode::Mailbox, PresentMode::Fifo], true), PresentMode::Fifo);
}

#[test]
fn a_suboptimal_swapchain_is_drawn_once_then_configured_again() {
    assert_eq!(steps(Got::Fresh), (true, false));
    assert_eq!(steps(Got::Stale), (true, true));
    assert_eq!(steps(Got::Outdated), (false, true));
    assert_eq!(steps(Got::Busy), (false, false));
}

#[test]
fn surface_formats_prefer_the_requested_format_then_the_first_srgb() {
    let desktop = Presentation::DESKTOP.format;
    assert_eq!(surface_format(&[Rgba8UnormSrgb, Bgra8UnormSrgb], desktop), Some(Bgra8UnormSrgb));
    assert_eq!(surface_format(&[Bgra8Unorm, Rgba8UnormSrgb], desktop), Some(Rgba8UnormSrgb));
    assert_eq!(surface_format(&[Rgb10a2Unorm], desktop), Some(Rgb10a2Unorm));
    assert_eq!(surface_format(&[], desktop), None);
    let mobile = Presentation::MOBILE.format;
    assert_eq!(surface_format(&[Rgba8Unorm, Rgba8UnormSrgb, Bgra8UnormSrgb], mobile), Some(Rgba8UnormSrgb));
}
