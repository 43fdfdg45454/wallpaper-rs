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
    let auto = wprs_core::QualityChoice::Auto;
    assert_ne!(own.key(auto), general.key(auto), "outputs with their own quality shared one page");
    let fixed = wprs_core::QualityChoice::Fixed(0);
    assert_eq!(own.key(fixed), general.key(fixed), "the same level ran in two helpers");
    assert_eq!(general.key(auto), page(false, json!({})).key(auto));
    assert!(own != general, "a change of quality did not reach the page");
}

#[test]
fn a_page_whose_level_changes_keeps_its_helper_unless_it_splits() {
    let spec = page(true, json!({}));
    let (auto, fixed) = (wprs_core::QualityChoice::Auto, wprs_core::QualityChoice::Fixed(0));
    let mut pages = std::collections::HashMap::new();
    let mut first = PageSlot::new(spec.clone(), Instant::now());
    first.next = 7;
    pages.insert(spec.key(auto), first);
    let mut wanted = keys::Wanted::new();
    wanted.insert(spec.key(fixed), (spec.clone(), Vec::new()));
    keys::rekey(&mut pages, &wanted);
    assert_eq!(pages.get(&spec.key(fixed)).map(|s| s.next), Some(7), "a new level restarted the page");
    wanted.insert(spec.key(auto), (spec.clone(), Vec::new()));
    keys::rekey(&mut pages, &wanted);
    assert_eq!(pages.len(), 1, "a split moved or dropped the running helper");
    assert_eq!(pages.get(&spec.key(fixed)).map(|s| s.next), Some(7));
    keys::rekey(&mut pages, &keys::Wanted::new());
    assert!(pages.is_empty(), "a page nothing shows kept its helper");
}
