use serde_json::{Value, json};
use wprs_core::layout::Monitor;
use wprs_core::spread::{bounds, checked, rects, simulated};

fn canvas(doc: &Value) -> [u32; 2] {
    let read = |i: usize| doc.pointer(&format!("/scene/canvas/size/{i}")).and_then(Value::as_u64);
    let side = |i, or| read(i).and_then(|v| u32::try_from(v).ok()).filter(|v| *v > 0).unwrap_or(or);
    [side(0, 1920), side(1, 1080)]
}

fn given(list: &Value) -> Result<Vec<Monitor>, String> {
    let list = list.as_array().ok_or("the screens are a list of {id, mm}")?;
    if list.len() > wprs_core::Layout::MAX_MONITORS {
        return Err(format!("a layout has 1 to {} screens", wprs_core::Layout::MAX_MONITORS));
    }
    let one = |m: &Value| -> Option<Monitor> {
        let mm: Vec<f64> = m["mm"].as_array()?.iter().map(Value::as_f64).collect::<Option<_>>()?;
        Some(Monitor { id: m["id"].as_str()?.to_owned(), mm: mm.try_into().ok()? })
    };
    let monitors = list.iter().map(one).collect::<Option<Vec<_>>>();
    checked(monitors.ok_or("every screen needs an id and mm as [x, y, width, height]")?)
}

pub fn spread(doc: &Value, q: &Value) -> Value {
    let spread = doc.pointer("/manifest/display/span").and_then(Value::as_str) == Some("all-outputs");
    let layout = q["layout"].as_str().unwrap_or_default();
    let bezel = q["bezel"].as_f64().unwrap_or(0.0);
    let monitors = match layout {
        "mine" => given(&q["monitors"]),
        name => {
            simulated(name, bezel).ok_or_else(|| format!("{name:?} is not dual, triple, vertical or mine"))
        }
    };
    let monitors = match monitors {
        Ok(m) => m,
        Err(e) => return json!({ "spread": spread, "error": e, "parts": [] }),
    };
    let size = canvas(doc);
    let screens: Vec<Value> = monitors.iter().map(|m| json!({ "id": m.id, "mm": m.mm })).collect();
    let parts: Vec<Value> =
        rects(&monitors, size).into_iter().map(|(id, r)| json!({ "id": id, "rect": r })).collect();
    json!({ "spread": spread, "canvas": size, "screens": screens, "bounds": bounds(&monitors), "parts": parts })
}
