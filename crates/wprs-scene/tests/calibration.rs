use std::cell::Cell;
use wprs_scene::{Calibration, Gpu, Tier, calibrate};

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
    assert_eq!(Tier::calibrated(&IRIS, 1.0), Tier::T1, "and lowers it one step at most");
    assert_eq!([Tier::calibrated(&IRIS, 9.0), Tier::calibrated(&uhd, 9.0)], [Tier::T1, Tier::T0]);
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
    let (first, fresh) =
        calibrate(Some(&path), ("iris", "mesa 26.1"), &IRIS, Calibration::FENCE, || measure(0.1));
    assert!(fresh && first.measured() && first.tier == Tier::T3);
    first.keep(&path).unwrap();
    let (again, fresh) =
        calibrate(Some(&path), ("iris", "mesa 26.1"), &IRIS, Calibration::FENCE, || measure(9.0));
    assert_eq!((again, fresh, measures.get()), (first.clone(), false, 1));
    let (newer, fresh) =
        calibrate(Some(&path), ("iris", "mesa 26.2"), &IRIS, Calibration::FENCE, || measure(0.4));
    assert_eq!((newer.tier, fresh, measures.get()), (Tier::T2, true, 2));
    let (other, _) = calibrate(Some(&path), ("arc", "mesa 26.1"), &IRIS, Calibration::FENCE, || measure(0.4));
    assert_eq!((other.tier, measures.get()), (Tier::T2, 3));
    std::fs::write(&path, "{ broken").unwrap();
    calibrate(Some(&path), ("iris", "mesa 26.1"), &IRIS, Calibration::FENCE, || measure(0.1));
    assert_eq!(measures.get(), 4);
    let software = Gpu { software: true, ..IRIS };
    let (slow, fresh) =
        calibrate(Some(&path), ("llvmpipe", "mesa"), &software, Calibration::FENCE, || measure(0.1));
    assert_eq!((slow.tier, slow.measured(), fresh, measures.get()), (Tier::T0, false, false, 4));
    let (failed, fresh) = calibrate(None, ("iris", "mesa"), &IRIS, Calibration::FENCE, || None);
    assert_eq!((failed.tier, failed.measured(), fresh), (Tier::detect(&IRIS), false, false));
}

#[test]
fn a_low_measure_is_taken_again_on_later_starts_and_the_best_one_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gpu-tier.json");
    let start = |ms: Option<f64>| {
        let (found, fresh) = calibrate(Some(&path), ("iris", "mesa"), &IRIS, Calibration::FENCE, || ms);
        if fresh {
            found.keep(&path).unwrap();
        }
        (found.tier, found.runs, fresh)
    };
    assert_eq!(start(Some(1.0)), (Tier::T1, 1, true), "a busy first start lowered the class");
    assert_eq!(start(Some(1.4)), (Tier::T1, 2, true));
    assert_eq!(start(None), (Tier::T1, 2, false));
    assert_eq!(start(Some(0.1)), (Tier::T3, 3, true), "a calmer start brings the class back");
    assert_eq!(start(Some(9.0)), (Tier::T3, 3, false));
    std::fs::remove_file(&path).unwrap();
    [1.0, 1.1, 1.2].into_iter().for_each(|ms| _ = start(Some(ms)));
    assert_eq!(start(Some(0.1)), (Tier::T1, 3, false), "three slow measures settle it");
}

#[test]
fn a_calibration_timed_by_the_fence_is_taken_again_with_gpu_timestamps() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gpu-tier.json");
    let (fence, _) = calibrate(Some(&path), ("iris", "mesa"), &IRIS, Calibration::FENCE, || Some(0.3));
    fence.keep(&path).unwrap();
    assert!(!fence.stamped());
    let (stamped, fresh) = calibrate(Some(&path), ("iris", "mesa"), &IRIS, Calibration::STAMPS, || Some(0.1));
    assert!(fresh && stamped.stamped() && stamped.unit_ms == 0.1);
}
