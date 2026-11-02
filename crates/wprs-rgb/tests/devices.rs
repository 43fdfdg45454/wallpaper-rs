mod server;

use server::{Shape, blob, serve};
use std::time::Duration;
use wprs_rgb::{Client, Error, STRIP, Strip, colors, controller, pattern, shrink};

const WAIT: Duration = Duration::from_secs(2);

fn shapes() -> Vec<Shape> {
    let keys = (6, Some((3, 2, vec![0, 1, 2, 3, 4, 5])));
    vec![
        Shape { name: "Example keyboard", kind: 5, zones: vec![keys, (2, None)] },
        Shape { name: "Example mouse", kind: 6, zones: vec![(1, None)] },
    ]
}

#[test]
fn devices_are_listed_and_painted_on_every_protocol_version() {
    for version in [None, Some(1), Some(3), Some(4), Some(5)] {
        let (address, painted) = serve(shapes(), version);
        let mut client = Client::connect(&address, WAIT).unwrap();
        assert_eq!(client.version, version.unwrap_or(0).min(4));
        let devices = client.devices().unwrap();
        let named: Vec<(&str, &str, u32)> =
            devices.iter().map(|d| (d.name.as_str(), d.kind, d.leds)).collect();
        assert_eq!(
            named,
            [("Example keyboard", "keyboard", 8), ("Example mouse", "mouse", 1)],
            "{version:?}"
        );
        assert_eq!(devices[0].zones[1].start, 6);
        assert_eq!(devices[0].zones[0].matrix.as_ref().map(|m| (m.0, m.1)), Some((3, 2)));
        client.direct(1).unwrap();
        client.paint(1, &[[10, 20, 30]]).unwrap();
        assert!(client.heard().is_ok());
        std::thread::sleep(Duration::from_millis(100));
        let got = painted.lock().unwrap().clone();
        assert_eq!(got[0], (1, 1100, vec![]));
        assert_eq!(got[1], (1, 1050, vec![10, 0, 0, 0, 1, 0, 10, 20, 30, 0]));
    }
}

#[test]
fn a_strip_spreads_over_matrices_and_lines() {
    let rgb = (0..4u8).flat_map(|x| (0..2u8).map(move |y| (x, y))).collect::<Vec<_>>();
    let mut pixels = vec![0u8; 4 * 2 * 3];
    for (x, y) in rgb {
        let at = ((y as usize) * 4 + x as usize) * 3;
        pixels[at..at + 3].copy_from_slice(&[x * 60, y * 200, 7]);
    }
    let strip = Strip::new(4, 2, pixels).unwrap();
    let device = controller(0, &blob(&shapes()[0], 4), 4).unwrap();
    let painted = colors(&device, &strip);
    assert_eq!(painted.len(), 8);
    assert_eq!(painted[0][1], 0);
    assert_eq!(painted[3][1], 200);
    assert!(painted[2][0] > painted[0][0]);
    assert_eq!((painted[6], painted[7]), ([30, 100, 7], [150, 100, 7]));
}

#[test]
fn pictures_shrink_to_the_strip_and_the_pattern_fills_it() {
    let big = vec![255u8; 640 * 40 * 4];
    let strip = shrink(640, 40, &big, 4).unwrap();
    assert_eq!((strip.width, strip.height), (STRIP.0, 4));
    assert!(strip.rgb.iter().all(|v| *v == 255));
    assert_eq!(shrink(10, 10, &big, 4), None);
    assert_eq!(shrink(0, 10, &[], 3), None);
    assert!(Strip::new(65, 1, vec![0; 195]).is_none());
    let rainbow = pattern(0.0);
    assert_eq!((rainbow.width, rainbow.height, &rainbow.rgb[..3]), (STRIP.0, 1, &[255, 0, 0][..]));
}

#[test]
fn broken_servers_and_addresses_are_refused() {
    assert!(matches!(Client::connect("no port", WAIT), Err(Error::Address(_))));
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().to_string();
    assert!(matches!(Client::connect(&closed, WAIT), Err(Error::Unreachable(_))));
    let short = blob(&shapes()[0], 3);
    assert_eq!(controller(0, &short[..short.len() / 2], 3), Err(Error::Broken));
    assert_eq!(controller(0, &short, 4), Err(Error::Broken));
    let (address, _) =
        serve(vec![Shape { name: "Example strip", kind: 99, zones: vec![(3, None)] }], Some(3));
    let mut client = Client::connect(&address, WAIT).unwrap();
    assert_eq!(client.devices().unwrap()[0].kind, "other");
}
