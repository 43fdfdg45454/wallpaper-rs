use crate::color::Manager;
use crate::state::State;
use cm::wp_color_management_surface_v1::WpColorManagementSurfaceV1 as CmSurface;
use cm::wp_color_manager_v1::{Event as Offered, Feature, Primaries, RenderIntent, TransferFunction as Tf};
use cm::wp_image_description_creator_params_v1::WpImageDescriptionCreatorParamsV1 as Creator;
use cm::wp_image_description_v1::{Event as Ready, WpImageDescriptionV1 as Description};
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use smithay_client_toolkit::reexports::protocols::wp::color_management::v1::client as cm;
use smithay_client_toolkit::shell::WaylandSurface;
use std::sync::atomic::{AtomicU8, Ordering};

const PARAMETRIC: u8 = 1;
const PQ: u8 = 2;
const WIDE: u8 = 4;
const PERCEPTUAL: u8 = 8;
const MASTERING: u8 = 16;
const SRGB: [u32; 8] = [640_000, 330_000, 300_000, 600_000, 150_000, 60_000, 312_700, 329_000];

#[derive(Debug, Default)]
pub struct Support(AtomicU8);

impl Support {
    fn add(&self, bit: u8) {
        self.0.fetch_or(bit, Ordering::Relaxed);
    }

    pub fn pq(&self) -> bool {
        let all = PARAMETRIC | PQ | WIDE | PERCEPTUAL;
        self.0.load(Ordering::Relaxed) & all == all
    }

    fn mastering(&self) -> bool {
        self.0.load(Ordering::Relaxed) & MASTERING != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mastered {
    pub max_cll: u32,
    pub max_fall: u32,
}

#[derive(Debug, Default)]
pub(crate) struct Own {
    surface: Option<CmSurface>,
    shown: Option<Mastered>,
}

pub(crate) struct Pending {
    surface: CmSurface,
    wl: WlSurface,
}

pub(crate) fn offered(support: &Support, event: Offered) {
    match event {
        Offered::SupportedIntent { render_intent: WEnum::Value(RenderIntent::Perceptual) } => {
            support.add(PERCEPTUAL)
        }
        Offered::SupportedFeature { feature: WEnum::Value(Feature::Parametric) } => support.add(PARAMETRIC),
        Offered::SupportedFeature { feature: WEnum::Value(Feature::SetMasteringDisplayPrimaries) } => {
            support.add(MASTERING)
        }
        Offered::SupportedTfNamed { tf: WEnum::Value(Tf::St2084Pq) } => support.add(PQ),
        Offered::SupportedPrimariesNamed { primaries: WEnum::Value(Primaries::Bt2020) } => support.add(WIDE),
        _ => {}
    }
}

pub(crate) fn describe(
    (manager, wl, qh): (&Manager, &WlSurface, &QueueHandle<State>),
    own: &mut Own,
    wanted: Option<Mastered>,
) {
    let support = manager.data::<Support>();
    let wanted = wanted.filter(|_| support.is_some_and(Support::pq));
    if own.shown == wanted {
        return;
    }
    own.shown = wanted;
    let Some(m) = wanted else {
        if let Some(surface) = own.surface.take() {
            surface.destroy();
            wl.commit();
        }
        return;
    };
    let surface = own.surface.get_or_insert_with(|| manager.get_surface(wl, qh, ())).clone();
    let creator = manager.create_parametric_creator(qh, ());
    creator.set_tf_named(Tf::St2084Pq);
    creator.set_primaries_named(Primaries::Bt2020);
    if support.is_some_and(Support::mastering) {
        let [rx, ry, gx, gy, bx, by, wx, wy] = SRGB.map(|v| v as i32);
        creator.set_mastering_display_primaries(rx, ry, gx, gy, bx, by, wx, wy);
        creator.set_mastering_luminance(50, m.max_cll.max(2));
    }
    creator.set_max_cll(m.max_cll);
    creator.set_max_fall(m.max_fall.min(m.max_cll));
    drop(creator.create(qh, Pending { surface, wl: wl.clone() }));
}

impl crate::Window {
    pub fn describes_pq(&self) -> bool {
        let crate::window::Native::Layer(parts, _) = &self.native else { return false };
        parts.color.as_ref().and_then(|m| m.data::<Support>()).is_some_and(Support::pq)
    }

    pub fn describe(&self, wanted: Option<Mastered>) {
        let crate::window::Native::Layer(parts, qh) = &self.native else { return };
        let Some(manager) = parts.color.as_ref() else { return };
        let mut own = parts.own.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        describe((manager, parts.layer.wl_surface(), qh), &mut own, wanted);
    }
}

impl Dispatch<Description, Pending> for State {
    fn event(
        _: &mut Self,
        desc: &Description,
        event: Ready,
        p: &Pending,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            Ready::Ready { .. } if p.surface.is_alive() => {
                p.surface.set_image_description(desc, RenderIntent::Perceptual);
                p.wl.commit();
            }
            Ready::Failed { msg, .. } => tracing::debug!("the desktop refused a PQ image description: {msg}"),
            _ => {}
        }
        desc.destroy();
    }
}

impl Dispatch<CmSurface, ()> for State {
    fn event(
        _: &mut Self,
        _: &CmSurface,
        _: cm::wp_color_management_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<Creator, ()> for State {
    fn event(
        _: &mut Self,
        _: &Creator,
        _: cm::wp_image_description_creator_params_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pq_description_needs_every_part_of_it() {
        let support = Support::default();
        offered(&support, Offered::SupportedFeature { feature: WEnum::Value(Feature::Parametric) });
        offered(&support, Offered::SupportedTfNamed { tf: WEnum::Value(Tf::St2084Pq) });
        offered(&support, Offered::SupportedPrimariesNamed { primaries: WEnum::Value(Primaries::Bt2020) });
        assert!(!support.pq(), "the perceptual intent is still missing");
        offered(&support, Offered::SupportedIntent { render_intent: WEnum::Value(RenderIntent::Perceptual) });
        assert!(support.pq() && !support.mastering());
        offered(&support, Offered::SupportedTfNamed { tf: WEnum::Value(Tf::Gamma22) });
        offered(
            &support,
            Offered::SupportedFeature { feature: WEnum::Value(Feature::SetMasteringDisplayPrimaries) },
        );
        assert!(support.mastering());
    }
}
