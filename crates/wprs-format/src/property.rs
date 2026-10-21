use crate::{Result, require};
use serde::{Deserialize, Serialize};

const MAX_TEXT: usize = 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Property {
    Color { default: String },
    Slider { min: f64, max: f64, step: f64, default: f64 },
    Bool { default: bool },
    Combo { options: Vec<String>, default: String },
    Text { default: String },
}

impl Property {
    pub(crate) fn check(&self, field: &str) -> Result<()> {
        match self {
            Property::Color { default } => {
                require(is_color(default), field, "default must be a #rrggbb color")
            }
            Property::Slider { min, max, step, default } => {
                let finite = [min, max, step, default].iter().all(|v| v.is_finite());
                let range = finite && min < max && *step > 0.0 && *step <= max - min;
                require(range, field, "expected finite values with min < max and 0 < step <= max - min")?;
                let steps = (default - min) / step;
                let aligned = (steps - steps.round()).abs() <= 1e-9 * steps.max(1.0);
                require(
                    (min..=max).contains(&default) && aligned,
                    field,
                    "default must lie on a step in range",
                )
            }
            Property::Bool { .. } => Ok(()),
            Property::Combo { options, default } => {
                let unique =
                    options.iter().enumerate().all(|(i, o)| !o.is_empty() && !options[..i].contains(o));
                require(unique && !options.is_empty(), field, "options must be non-empty and unique")?;
                require(options.contains(default), field, "default must be one of the options")
            }
            Property::Text { default } => require(
                default.chars().count() <= MAX_TEXT,
                field,
                "default must have at most 1024 characters",
            ),
        }
    }
}

fn is_color(text: &str) -> bool {
    text.strip_prefix('#').is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}
