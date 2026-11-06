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
    let (hexes, lists) = ([base, value].map(Value::is_string), [base, value].map(Value::is_array));
    let wide = [a.len(), b.len()];
    let colored = hexes.contains(&true) || wide.contains(&4) || wide == [3, 4] || wide == [4, 3];
    for side in [&mut a, &mut b] {
        match side.len() {
            3 if colored => side.push(1.0),
            1 if colored => *side = vec![side[0], side[0], side[0], 0.0],
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
    Some(match (colored && !lists.contains(&true), sum.as_slice()) {
        (true, _) => json!(Color(std::array::from_fn(|i| sum[i] as f32)).hex()),
        (_, [one]) => json!(one),
        _ => json!(sum),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_values_add_up_as_the_scene_adds_them() {
        assert_eq!(added(&json!("#404000"), &json!(0.25)), Some(json!("#808040")));
        assert_eq!(added(&json!(0.25), &json!([0.25, 0.5, 0.0, 0.5])), Some(json!([0.5, 0.75, 0.25, 0.5])));
        assert_eq!(added(&json!([1, 2]), &json!(3)), Some(json!([4.0, 5.0])));
        assert_eq!(added(&json!([1, 2, 3]), &json!(1)), Some(json!([2.0, 3.0, 4.0])));
        assert_eq!(added(&json!("#ff000080"), &json!("#000000")), Some(json!("#ff0000")));
        assert_eq!(added(&json!([1, 2]), &json!("#ffffff")), None);
    }
}
