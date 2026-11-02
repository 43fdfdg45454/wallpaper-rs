use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_core::QualityChoice;
use wprs_scene::{Budget, Governor, Heat, Quality, Runtime, SCHEMA, Scene, Tier, Why};

fn runtime(layers: usize) -> Runtime {
    let blur = |i: usize| json!({ "id": format!("b{i}"), "type": "adjustment", "effects": [{ "effect": "blur", "params": { "radius": 30 } }] });
    let layers: Vec<_> = (0..layers).map(blur).collect();
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [960, 540] }, "layers": layers });
    Runtime::new(Arc::new(Scene::from_json(&text.to_string()).unwrap()), Default::default())
}

fn strains(tier: Tier) -> [Budget; 5] {
    let mains = Budget::new(tier, false, 60);
    let battery = Budget::new(tier, true, 60);
    let (saver, warm, hot) = (
        Budget { saver: true, ..mains },
        Budget { heat: Heat::Warm, ..mains },
        Budget { heat: Heat::Hot, ..battery },
    );
    [mains, battery, saver, warm, hot]
}

fn levels(choice: &str, tier: Tier, runtime: &Runtime) -> Vec<(Quality, Why)> {
    let choice = QualityChoice::parse(choice).unwrap();
    strains(tier).into_iter().map(|b| Budget { choice, ..b }.level(runtime)).collect()
}

#[test]
fn auto_keeps_the_automatic_choice_exactly() {
    let (light, heavy) = (runtime(0), runtime(6));
    for tier in [Tier::T0, Tier::T1, Tier::T2, Tier::T3] {
        for scene in [&light, &heavy] {
            let auto: Vec<_> = strains(tier).into_iter().map(|b| b.auto(scene)).collect();
            assert_eq!(levels("auto", tier, scene), auto);
            assert!(auto.iter().all(|(_, why)| *why != Why::User));
        }
    }
    assert_eq!(Budget::new(Tier::T1, true, 60).quality(&light), Quality::Q1);
}

#[test]
fn a_fixed_level_holds_on_battery_under_heat_and_over_cost() {
    let heavy = runtime(6);
    for tier in [Tier::T0, Tier::T2, Tier::T3] {
        assert_eq!(levels("q3", tier, &heavy), vec![(Quality::Q3, Why::User); 5]);
        assert_eq!(levels("q0", tier, &runtime(0)), vec![(Quality::Q0, Why::User); 5]);
        let power = Budget { choice: QualityChoice::Fixed(2), heat: Heat::Hot, ..Budget::new(tier, true, 0) };
        assert_eq!((power.quality(&heavy), power.steady()), (Quality::Q2, Quality::Q2));
    }
}

#[test]
fn a_floor_only_stops_the_automatic_choice_from_going_lower() {
    let (light, heavy) = (runtime(0), runtime(6));
    assert_eq!(levels("min-q2", Tier::T3, &light)[0], (Quality::Q3, Why::Full), "it can still go higher");
    let floored = levels("min-q2", Tier::T3, &light);
    assert_eq!(
        floored[1..],
        [
            (Quality::Q2, Why::Battery),
            (Quality::Q2, Why::User),
            (Quality::Q2, Why::Heat),
            (Quality::Q2, Why::User)
        ]
    );
    assert_eq!(levels("min-q1", Tier::T0, &heavy), vec![(Quality::Q1, Why::User); 5]);
    assert_eq!(levels("min-q2", Tier::T2, &heavy)[0], (Quality::Q2, Why::User));
    let hot =
        Budget { choice: QualityChoice::AtLeast(1), heat: Heat::Hot, ..Budget::new(Tier::T3, false, 0) };
    assert_eq!(hot.steady(), Quality::Q1);
}

#[test]
fn chosen_levels_above_what_the_card_holds_report_their_cost() {
    let heavy = runtime(6);
    let auto = Budget::new(Tier::T0, false, 60);
    assert_eq!(auto.over(&heavy, Quality::Q3), None, "the automatic choice needs no note");
    let fixed = Budget { choice: QualityChoice::Fixed(3), ..auto };
    let (cost, budget) = fixed.over(&heavy, Quality::Q3).unwrap();
    assert!(cost > budget && budget == Tier::T0.budget(), "{cost} {budget}");
    assert_eq!(
        Budget { choice: QualityChoice::Fixed(3), ..Budget::new(Tier::T3, false, 60) }
            .over(&runtime(0), Quality::Q3),
        None
    );
}

#[test]
fn slow_frames_never_take_the_level_below_the_floor() {
    let (mut g, mut at) = (Governor::default(), Instant::now());
    let slow = |g: &mut Governor, at: &mut Instant, ceiling| {
        (0..3600).for_each(|_| {
            *at += Duration::from_micros(16_667);
            g.feed(*at, 40.0, |_| (8.3, 1.0), ceiling, true);
        });
    };
    g.hold(Quality::Q2);
    slow(&mut g, &mut at, Quality::Q3);
    assert_eq!(g.level((Quality::Q3, Why::Full)), (Quality::Q2, Why::Slow));
    g.hold(Quality::Q3);
    slow(&mut g, &mut at, Quality::Q3);
    assert_eq!(g.level((Quality::Q3, Why::User)), (Quality::Q3, Why::User), "a fixed level never moves");
    g.hold(Quality::Q0);
    slow(&mut g, &mut at, Quality::Q3);
    assert_eq!(g.level((Quality::Q3, Why::Full)).0, Quality::Q0, "without a floor it goes to the bottom");
}
