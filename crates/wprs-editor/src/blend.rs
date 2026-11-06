use serde_json::{Value, json};
use wprs_core::message::{Message, said};
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

pub(crate) fn fitting(pointer: &str, value: &Value, like: &Value) -> Message {
    let shape = match like {
        Value::Number(_) => said("editor.shape_number", "a number such as 0.5", &[]),
        Value::Bool(_) => said("editor.shape_switch", "true or false", &[]),
        Value::String(text) if Color::parse(text).is_some() => {
            said("editor.shape_color", "a color such as \"#ff8800\" or [1, 0.5, 0]", &[])
        }
        Value::String(_) => said("editor.shape_text", "a text", &[]),
        Value::Array(list) => {
            let ones = vec!["1"; list.len()].join(", ");
            said("editor.shape_list", "a list of %1 numbers such as [%2]", &[&list.len(), &ones])
        }
        _ => said("editor.shape_like", "a value like %1", &[&like]),
    };
    let unfit = said(
        "editor.keyframe_unfit",
        "%1 cannot take the keyframe value %2; give %3",
        &[&pointer, &value, &shape],
    );
    unfit.with(3, shape)
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
