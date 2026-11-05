use crate::{Error, choice, said, settings};
use serde::Deserialize;
use serde_json::{Value, json};
use wprs_core::filter::{Filter, LOOKS, RANGES};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Request {
    Filter { lock: bool },
    SetFilter { lock: bool, filter: Option<Value> },
}

pub(crate) fn request(request: Request) -> Result<Value, Error> {
    match request {
        Request::Filter { lock } => Ok(shown(lock)),
        Request::SetFilter { lock, filter } => {
            let chosen = filter.map(read).transpose()?;
            settings::update(|config| {
                match (lock, chosen) {
                    (false, chosen) => config.default.filter = chosen.filter(|f| !f.is_identity()),
                    (true, Some(chosen)) => config.set_filter(choice::LOCK, chosen),
                    (true, None) => drop(config.outputs.get_mut(choice::LOCK).map(|o| o.filter.take())),
                }
                Ok(())
            })?;
            Ok(shown(lock))
        }
    }
}

fn read(value: Value) -> Result<Filter, Error> {
    let filter: Filter = serde_json::from_value(value)
        .map_err(|e| said("android.filter_value", "the filter cannot be read: %1", &[&e]))?;
    if !filter.valid() {
        let ranges = RANGES.iter().map(|(name, lo, hi)| format!("{name} {lo} to {hi}"));
        let ranges = ranges.collect::<Vec<_>>().join(", ");
        return Err(said("android.filter_range", "the filter takes %1", &[&ranges]));
    }
    Ok(filter.sanitized())
}

fn shown(lock: bool) -> Value {
    let config = settings::config();
    let own = if lock {
        config.outputs.get(choice::LOCK).is_some_and(|o| o.filter.is_some())
    } else {
        config.default.filter.is_some()
    };
    let filter = config.filter(choice::screen(lock));
    let value = serde_json::to_value(filter).unwrap_or(Value::Null);
    let fields = RANGES.iter().map(|(key, min, max)| {
        let places = if max - min > 100.0 { 0 } else { 2 };
        json!({ "key": key, "min": min, "max": max, "decimals": places, "value": value[key] })
    });
    let looks = LOOKS.iter().map(|look| json!(look));
    json!({ "filter": value, "own": own, "fields": fields.collect::<Vec<_>>(), "looks": looks.collect::<Vec<_>>() })
}
