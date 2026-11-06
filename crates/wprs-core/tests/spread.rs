use wprs_core::layout::Monitor;
use wprs_core::spread::{LAYOUTS, SCREEN_MM, bounds, checked, rects, screen, simulated, sized};

fn close(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.05)
}

#[test]
fn simulated_layouts_put_frames_between_their_screens() {
    let dual = simulated("dual", 10.0).unwrap();
    assert_eq!(dual.len(), 2);
    assert_eq!(dual[1].mm, [547.0, 0.0, 527.0, 296.0]);
    assert_eq!(simulated("triple", 0.0).unwrap()[2].mm[0], 1054.0);
    assert_eq!(simulated("vertical", 5.0).unwrap()[1].mm, [0.0, 306.0, 527.0, 296.0]);
    assert_eq!(simulated("dual", f64::NAN).unwrap()[1].mm[0], 527.0);
    assert_eq!(simulated("dual", 1e9).unwrap()[1].mm[0], 727.0);
    assert!(simulated("quad", 0.0).is_none() && simulated("", 0.0).is_none());
    assert!(LAYOUTS.iter().all(|name| simulated(name, 0.0).is_some()));
}

#[test]
fn each_screen_shows_its_part_of_the_canvas_and_the_frames_hide_theirs() {
    let dual = simulated("dual", 10.0).unwrap();
    let parts = rects(&dual, [2000, 500]);
    let px = 500.0 / 296.0;
    let left = (2000.0 - 1074.0 * px) / 2.0;
    assert_eq!(parts[0].0, "screen-1");
    assert!(close(parts[0].1, [left, 0.0, 527.0 * px, 500.0]), "{parts:?}");
    assert!(close(parts[1].1, [left + 547.0 * px, 0.0, 527.0 * px, 500.0]), "{parts:?}");
    let tall = rects(&simulated("vertical", 0.0).unwrap(), [1920, 1080]);
    assert!(tall[1].1[1] > 0.0 && tall[0].1[1] >= 0.0, "{tall:?}");
    assert!(tall[0].1[1] < tall[1].1[1] && (tall[0].1[2] - tall[1].1[2]).abs() < 1e-3);
    assert_eq!(bounds(&dual), [0.0, 0.0, 1074.0, 296.0]);
    assert_eq!(bounds(&[]), [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn hand_made_layouts_need_named_screens_with_real_sizes() {
    let one = |id: &str, mm: [f64; 4]| vec![Monitor { id: id.into(), mm }];
    assert!(checked(one("left", [0.0, 0.0, 500.0, 300.0])).is_ok());
    assert!(checked(Vec::new()).is_err());
    assert!(checked(one("", [0.0, 0.0, 500.0, 300.0])).is_err());
    assert!(checked(one("a\nb", [0.0, 0.0, 500.0, 300.0])).is_err());
    assert!(checked(one("left", [0.0, 0.0, 0.0, 300.0])).is_err());
    assert!(checked(one("left", [f64::NAN, 0.0, 500.0, 300.0])).is_err());
    assert!(checked(one("left", [0.0, 0.0, 20_000.0, 300.0])).is_err());
    let many = (0..65).map(|i| Monitor { id: format!("s{i}"), mm: [0.0, 0.0, 1.0, 1.0] }).collect();
    assert!(checked(many).is_err());
}

#[test]
fn simulated_screens_take_the_size_the_author_gives() {
    let wide = sized("dual", 5.0, [800.0, 335.0]).unwrap();
    assert_eq!(wide[1].mm, [810.0, 0.0, 800.0, 335.0]);
    assert_eq!(sized("triple", 0.0, SCREEN_MM), simulated("triple", 0.0));
    for bad in [[49.0, 300.0], [600.0, 3001.0], [f64::NAN, 300.0], [600.0, f64::INFINITY], [-600.0, 300.0]] {
        assert!(sized("dual", 0.0, bad).is_none(), "{bad:?}");
    }
    let near = |a: [f64; 2], b: [f64; 2]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.5);
    assert!(near(screen("24in").unwrap(), [531.3, 298.9]));
    assert!(near(screen(" 34in@21:9 ").unwrap(), [793.8, 340.2]));
    assert!(near(screen("27in@16:10").unwrap(), [581.5, 363.4]));
    assert_eq!(screen("600x340").unwrap(), [600.0, 340.0]);
    assert_eq!(screen("600 × 340").unwrap(), [600.0, 340.0]);
    for bad in [
        "", "24", "in", "24in@16", "24in@0:9", "24in16:9", "x340", "600x", "-600x340", "nanx340", "1in",
        "200in", "600x40",
    ] {
        assert!(screen(bad).is_err(), "{bad:?}");
    }
    assert!(screen("1in").unwrap_err().contains("50 to 3000 mm"));
}
