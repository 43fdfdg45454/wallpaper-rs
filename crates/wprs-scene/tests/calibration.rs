use std::cell::Cell;
use wprs_scene::{Gpu, Tier, calibrate};

const IRIS: Gpu = Gpu {
    name: "Intel(R) Iris(R) Xe Graphics",
    software: false,
    discrete: false,
    compute: true,
    max_texture: 16384,
    memory_mib: 16384,
};

#[test]
fn the_measured_time_of_a_unit_picks_the_tier_within_bounds() {
    assert_eq!([0.05, 0.3, 1.5, 4.0].map(Tier::measured), [Tier::T3, Tier::T2, Tier::T1, Tier::T0]);
    let uhd = Gpu { name: "Intel(R) UHD Graphics 620", ..IRIS };
    assert_eq!(Tier::calibrated(&uhd, 0.01), Tier::T2, "a measure raises the detected tier one step at most");
    assert_eq!(Tier::calibrated(&IRIS, 1.0), Tier::T1, "and lowers it as far as it shows");
    assert_eq!(Tier::calibrated(&Gpu { memory_mib: 3072, ..IRIS }, 0.01), Tier::T1, "memory still caps it");
    assert_eq!(Tier::calibrated(&Gpu { software: true, ..IRIS }, 0.01), Tier::T0);
}

#[test]
fn a_calibration_is_kept_until_the_adapter_or_driver_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wallpaper-rs").join("gpu-tier.json");
    let measures = Cell::new(0);
    let measure = |ms: f64| {
        measures.set(measures.get() + 1);
        Some(ms)
    };
    let (first, fresh) = calibrate(Some(&path), ("iris", "mesa 26.1"), &IRIS, || measure(0.1));
    assert!(fresh && first.measured() && first.tier == Tier::T3);
    first.keep(&path).unwrap();
    let (again, fresh) = calibrate(Some(&path), ("iris", "mesa 26.1"), &IRIS, || measure(9.0));
    assert_eq!((again, fresh, measures.get()), (first.clone(), false, 1));
    let (newer, fresh) = calibrate(Some(&path), ("iris", "mesa 26.2"), &IRIS, || measure(0.4));
    assert_eq!((newer.tier, fresh, measures.get()), (Tier::T2, true, 2));
    let (other, _) = calibrate(Some(&path), ("arc", "mesa 26.1"), &IRIS, || measure(0.4));
    assert_eq!((other.tier, measures.get()), (Tier::T2, 3));
    std::fs::write(&path, "{ broken").unwrap();
    calibrate(Some(&path), ("iris", "mesa 26.1"), &IRIS, || measure(0.1));
    assert_eq!(measures.get(), 4);
    let software = Gpu { software: true, ..IRIS };
    let (slow, fresh) = calibrate(Some(&path), ("llvmpipe", "mesa"), &software, || measure(0.1));
    assert_eq!((slow.tier, slow.measured(), fresh, measures.get()), (Tier::T0, false, false, 4));
    let (failed, fresh) = calibrate(None, ("iris", "mesa"), &IRIS, || None);
    assert_eq!((failed.tier, failed.measured(), fresh), (Tier::detect(&IRIS), false, false));
}
