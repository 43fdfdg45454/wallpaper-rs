use super::tests::session;
use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver as Heard, channel};

fn text(out: &mut Vec<u8>, s: &str) {
    out.extend((s.len() as u16 + 1).to_le_bytes());
    out.extend(s.as_bytes());
    out.push(0);
}

fn device() -> Vec<u8> {
    let mut out = 5u32.to_le_bytes().to_vec();
    (0..5).for_each(|i| text(&mut out, ["Example keyboard", "Example", "", "", ""][i]));
    out.extend(1u16.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    text(&mut out, "Breathing");
    out.extend([0; 36]);
    out.extend(0u16.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    text(&mut out, "led");
    out.extend(0u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend([9, 8, 7, 0]);
    [((out.len() + 4) as u32).to_le_bytes().to_vec(), out].concat()
}

fn server() -> (String, Heard<u32>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let (sent, heard) = channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = [0u8; 16];
        while stream.read_exact(&mut head).is_ok() {
            let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
            let mut data = vec![0; word(12) as usize];
            stream.read_exact(&mut data).ok();
            let answer = match word(8) {
                40 => 0u32.to_le_bytes().to_vec(),
                0 => 1u32.to_le_bytes().to_vec(),
                1 => device(),
                50 => continue,
                id => {
                    sent.send(id).ok();
                    continue;
                }
            };
            stream
                .write_all(&[&wprs_rgb::header(0, word(8), answer.len() as u32)[..], &answer].concat())
                .ok();
        }
    });
    (address, heard)
}

fn sent(heard: &Heard<u32>) -> Vec<u32> {
    std::iter::from_fn(|| heard.recv_timeout(Duration::from_millis(500)).ok()).collect()
}

#[test]
fn the_devices_get_their_own_mode_back_once_nothing_sends_colors() {
    let (address, heard) = server();
    let mut s = session("TEST-RESTORE");
    s.rgb = Rgb { enabled: true, server: address, ..Rgb::default() };
    s.paint(&Strip::new(1, 1, vec![0, 255, 0]).unwrap(), None);
    assert_eq!(sent(&heard), [1100, 1050], "the device was not taken for direct colors");
    assert_eq!(lock(&s.status).saved[0].mode.as_ref().map(|m| m.name.as_str()), Some("Breathing"));
    s.paint(&Strip::new(1, 1, vec![0, 0, 255]).unwrap(), None);
    assert_eq!(sent(&heard), [1050], "a taken device was taken again");
    s.forget(Some("OTHER"));
    assert!(sent(&heard).is_empty(), "another source gave the devices back");
    s.forget(Some("TEST-RESTORE"));
    assert_eq!(sent(&heard), [1101], "the device did not get its mode back");
    assert!(lock(&s.status).saved.is_empty());
    s.forget(None);
    assert!(sent(&heard).is_empty(), "the mode was given back twice");
}

#[test]
fn turning_the_lights_off_gives_the_devices_back() {
    let (address, heard) = server();
    let mut s = session("TEST-OFF");
    s.rgb = Rgb { enabled: true, server: address, ..Rgb::default() };
    s.paint(&Strip::new(1, 1, vec![255, 0, 0]).unwrap(), None);
    assert_eq!(sent(&heard), [1100, 1050]);
    s.forget(None);
    assert_eq!(sent(&heard), [1101]);
}

#[test]
fn a_device_left_alone_is_never_taken_and_placing_it_again_needs_no_new_connection() {
    let (address, heard) = server();
    let mut s = session("TEST-PLACED");
    s.rgb = Rgb { enabled: true, server: address, ..Rgb::default() };
    s.rgb.set_place("Example keyboard", "off").unwrap();
    s.paint(&Strip::new(1, 1, vec![255, 0, 0]).unwrap(), None);
    assert!(sent(&heard).is_empty(), "a device left alone was taken or painted");
    s.last = Some(("TEST-PLACED".into(), Strip::new(1, 1, vec![255, 0, 0]).unwrap()));
    let mut placed = s.rgb.clone();
    placed.set_place("Example keyboard", "0 0 0.5 1 down").unwrap();
    s.settle(placed);
    assert!(s.client.is_some(), "placing a device dropped the connection");
}
