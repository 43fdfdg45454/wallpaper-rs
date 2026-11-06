use super::tests::page;
use super::view::ViewSlot;
use super::*;
use std::sync::Arc;

fn slot(rgb: bool) -> PageSlot {
    let mut spec = page(true, json!({}));
    Arc::get_mut(&mut spec.pack).expect("a page of its own").uses[3] = rgb;
    let mut slot = PageSlot::new(spec, Instant::now());
    for (name, id) in [("preview:lit", 1), ("TEST-LIT-OFF", 2), ("TEST-LIT-CLOSED", 3)] {
        let mut view = ViewSlot::new(id, (8, 8));
        view.opened = id != 3;
        slot.views.insert(name.into(), view);
    }
    slot
}

#[test]
fn a_page_may_light_only_in_previews_or_with_the_permission_the_grant_and_lights_on() {
    let mut lit = slot(true);
    let told = feed::lights(&mut lit);
    assert_eq!(told.len(), 2, "a closed view was told: {told:?}");
    assert!(told.contains(&ToWeb::Lights { view: 1, on: true }), "a preview could not light: {told:?}");
    assert!(told.contains(&ToWeb::Lights { view: 2, on: false }), "an output without the grant could light");
    assert!(feed::lights(&mut lit).is_empty(), "unchanged lights were told again");
    let mut plain = slot(false);
    assert!(feed::lights(&mut plain).iter().all(|m| matches!(m, ToWeb::Lights { on: false, .. })));
}

#[test]
fn a_page_takes_the_quality_its_output_chose_and_shares_a_helper_only_with_the_same_choice() {
    let power = super::super::power::Power::new(wprs_scene::Tier::T3, false, 0);
    let mut own = page(true, json!({}));
    own.quality = Some(wprs_core::QualityChoice::Fixed(0));
    let general = page(true, json!({}));
    assert_eq!(level(power, &own), wprs_scene::Quality::Q0, "the quality of the output was ignored");
    assert_eq!(level(power, &general), power.steady());
    assert_ne!(own.key(), general.key(), "outputs with their own quality shared one page");
    assert_eq!(general.key(), page(false, json!({})).key());
    assert!(own != general, "a change of quality did not reach the page");
}
