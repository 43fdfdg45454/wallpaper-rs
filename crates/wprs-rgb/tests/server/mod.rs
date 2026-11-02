use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

pub type Painted = Arc<Mutex<Vec<(u32, u32, Vec<u8>)>>>;

fn text(out: &mut Vec<u8>, s: &str) {
    out.extend((s.len() as u16 + 1).to_le_bytes());
    out.extend(s.as_bytes());
    out.push(0);
}

fn word(out: &mut Vec<u8>, n: u32) {
    out.extend(n.to_le_bytes());
}

pub type Matrix = (u32, u32, Vec<u32>);

pub struct Shape {
    pub name: &'static str,
    pub kind: u32,
    pub zones: Vec<(u32, Option<Matrix>)>,
}

pub fn blob(shape: &Shape, version: u32) -> Vec<u8> {
    let mut out = Vec::new();
    word(&mut out, shape.kind);
    text(&mut out, shape.name);
    let texts = if version >= 1 { 5 } else { 4 };
    (0..texts).for_each(|i| text(&mut out, ["Example", "A test device", "1.0", "", "local"][i]));
    out.extend(1u16.to_le_bytes());
    word(&mut out, 0);
    text(&mut out, "Direct");
    let words = 4 + if version >= 3 { 2 } else { 0 } + 3 + if version >= 3 { 1 } else { 0 } + 2;
    (0..words).for_each(|_| word(&mut out, 0));
    out.extend(0u16.to_le_bytes());
    out.extend((shape.zones.len() as u16).to_le_bytes());
    for (i, (leds, matrix)) in shape.zones.iter().enumerate() {
        text(&mut out, &format!("zone {i}"));
        [1, 0, *leds, *leds].into_iter().for_each(|n| word(&mut out, n));
        match matrix {
            Some((w, h, map)) => {
                out.extend(((8 + 4 * map.len()) as u16).to_le_bytes());
                word(&mut out, *h);
                word(&mut out, *w);
                map.iter().for_each(|n| word(&mut out, *n));
            }
            None => out.extend(0u16.to_le_bytes()),
        }
        if version >= 4 {
            out.extend(1u16.to_le_bytes());
            text(&mut out, "segment");
            [0, 0, *leds].into_iter().for_each(|n| word(&mut out, n));
        }
    }
    let total: u32 = shape.zones.iter().map(|(n, _)| n).sum();
    out.extend((total as u16).to_le_bytes());
    (0..total).for_each(|i| {
        text(&mut out, &format!("led {i}"));
        word(&mut out, 0);
    });
    out.extend((total as u16).to_le_bytes());
    (0..total).for_each(|_| word(&mut out, 0));
    let mut sized = ((out.len() + 4) as u32).to_le_bytes().to_vec();
    sized.extend(out);
    sized
}

fn reply(device: u32, id: u32, data: &[u8]) -> Vec<u8> {
    let mut out = wprs_rgb::header(device, id, data.len() as u32).to_vec();
    out.extend_from_slice(data);
    out
}

pub fn serve(shapes: Vec<Shape>, version: Option<u32>) -> (String, Painted) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let painted: Painted = Arc::default();
    let kept = painted.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut agreed = 0;
            loop {
                let mut head = [0u8; 16];
                if stream.read_exact(&mut head).is_err() {
                    break;
                }
                let at = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().unwrap());
                let mut data = vec![0; at(12) as usize];
                stream.read_exact(&mut data).unwrap();
                let (device, id) = (at(4), at(8));
                let answer = match id {
                    40 => version.map(|v| {
                        agreed = v.min(u32::from_le_bytes(data[..4].try_into().unwrap()));
                        reply(0, 40, &v.to_le_bytes())
                    }),
                    0 => Some(reply(0, 0, &(shapes.len() as u32).to_le_bytes())),
                    1 => Some(
                        [reply(0, 100, &[]), reply(device, 1, &blob(&shapes[device as usize], agreed))]
                            .concat(),
                    ),
                    1050 | 1100 => {
                        kept.lock().unwrap().push((device, id, data));
                        None
                    }
                    _ => None,
                };
                answer.into_iter().for_each(|a| stream.write_all(&a).unwrap());
            }
        }
    });
    (address, painted)
}
