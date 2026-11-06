use serde_json::{Value, json};
use wprs_scene::Color;

pub(crate) fn parts(v: &Value) -> Option<Vec<f64>> {
    match v {
        Value::Number(n) => n.as_f64().map(|n| vec![n]),
        Value::Array(list) => list.iter().map(Value::as_f64).collect(),
        Value::String(text) => Color::parse(text).map(|c| c.0.map(f64::from).to_vec()),
        _ => None,
    }
}

pub(crate) fn added(base: &Value, value: &Value) -> Option<Value> {
    let (mut a, mut b) = (parts(base)?, parts(value)?);
    let hexes = [base, value].map(Value::is_string);
    let colored = hexes.contains(&true) || [a.len(), b.len()] == [3, 4] || [a.len(), b.len()] == [4, 3];
    for side in [&mut a, &mut b] {
        match side.len() {
            3 if colored => side.push(1.0),
            1 if colored => return None,
            _ => {}
        }
    }
    let n = a.len().max(b.len());
    for side in [&mut a, &mut b] {
        if side.len() == 1 {
            *side = vec![side[0]; n];
        }
    }
    let sum: Vec<f64> = (a.len() == b.len()).then(|| a.iter().zip(&b).map(|(a, b)| a + b).collect())?;
    let mut sum: Vec<f64> = sum.into_iter().map(|x| (x * 1e6).round() / 1e6).collect();
    if colored {
        sum[3] = sum[3].clamp(0.0, 1.0);
    }
    Some(match (hexes, sum.as_slice()) {
        ([true, true], _) => json!(Color(std::array::from_fn(|i| sum[i] as f32)).hex()),
        (_, [one]) => json!(one),
        _ => json!(sum),
    })
}
