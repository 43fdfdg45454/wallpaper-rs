use crate::state::State;
use cm::wp_color_management_surface_feedback_v1::Event as Preferred;
pub(crate) use cm::wp_color_management_surface_feedback_v1::WpColorManagementSurfaceFeedbackV1 as Feedback;
use cm::wp_color_manager_v1::TransferFunction as Tf;
pub(crate) use cm::wp_color_manager_v1::WpColorManagerV1 as Manager;
use cm::wp_image_description_info_v1::{Event as Info, WpImageDescriptionInfoV1 as Described};
use cm::wp_image_description_v1::{Event as Ready, WpImageDescriptionV1 as Description};
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{Connection, Dispatch, QueueHandle, WEnum};
use smithay_client_toolkit::reexports::protocols::wp::color_management::v1::client as cm;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    pub peak_nits: f32,
    pub reference_nits: f32,
    pub hdr: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Gathered {
    luminances: Option<(u32, u32)>,
    target: Option<u32>,
    wide: bool,
}

pub(crate) fn bind(globals: &GlobalList, qh: &QueueHandle<State>) -> Option<Manager> {
    globals
        .bind::<Manager, _, _>(qh, 1..=1, ())
        .inspect_err(|e| tracing::debug!("no color management: {e}"))
        .ok()
}

pub(crate) fn watch(
    manager: Option<&Manager>,
    surface: &WlSurface,
    qh: &QueueHandle<State>,
    name: &str,
) -> Option<Feedback> {
    let feedback = manager?.get_surface_feedback(surface, qh, name.to_owned());
    drop(feedback.get_preferred(qh, name.to_owned()));
    Some(feedback)
}

pub(crate) fn light(found: Gathered) -> Option<Light> {
    let (max, reference) = found.luminances?;
    let peak = found.target.unwrap_or(max).min(max.max(1));
    let (peak_nits, reference_nits) = (peak as f32, reference.max(1) as f32);
    Some(Light { peak_nits, reference_nits, hdr: found.wide || peak_nits > reference_nits * 1.05 })
}

impl Dispatch<Manager, ()> for State {
    fn event(
        _: &mut Self,
        _: &Manager,
        _: cm::wp_color_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<Feedback, String> for State {
    fn event(
        _: &mut Self,
        feedback: &Feedback,
        event: Preferred,
        name: &String,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let Preferred::PreferredChanged { .. } = event {
            drop(feedback.get_preferred(qh, name.clone()));
        }
    }
}

impl Dispatch<Description, String> for State {
    fn event(
        _: &mut Self,
        desc: &Description,
        event: Ready,
        name: &String,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let Ready::Ready { .. } = event {
            drop(desc.get_information(qh, name.clone()));
        }
        desc.destroy();
    }
}

impl Dispatch<Described, String> for State {
    fn event(
        state: &mut Self,
        _: &Described,
        event: Info,
        name: &String,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let found = state.lights.entry(name.clone()).or_default();
        match event {
            Info::Luminances { max_lum, reference_lum, .. } => {
                found.luminances = Some((max_lum, reference_lum))
            }
            Info::TargetLuminance { max_lum, .. } => found.target = Some(max_lum),
            Info::TfNamed { tf: WEnum::Value(Tf::St2084Pq | Tf::Hlg | Tf::ExtLinear) } => found.wide = true,
            Info::Done => {
                let lit = state.lights.remove(name).and_then(light);
                let screen = state.screens.iter().find(|s| s.name == *name);
                if let Some(parts) = screen.map(|s| &s.parts) {
                    *parts.light.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = lit;
                    tracing::debug!(output = name, ?lit, "the display told its light");
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_display_light_comes_from_its_luminances() {
        let sdr = Gathered { luminances: Some((80, 80)), target: None, wide: false };
        assert_eq!(light(sdr), Some(Light { peak_nits: 80.0, reference_nits: 80.0, hdr: false }));
        let hdr = Gathered { luminances: Some((10_000, 203)), target: Some(800), wide: true };
        assert_eq!(light(hdr), Some(Light { peak_nits: 800.0, reference_nits: 203.0, hdr: true }));
        let odd = Gathered { luminances: Some((400, 0)), target: Some(9_999), wide: false };
        assert_eq!(light(odd), Some(Light { peak_nits: 400.0, reference_nits: 1.0, hdr: true }));
        assert_eq!(light(Gathered::default()), None);
    }
}
