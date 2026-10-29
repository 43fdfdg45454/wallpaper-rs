use crate::easing::FAMILIES;
use crate::{Ease, Handles, Key, Property, Scene, Text};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde_json::{Value as Json, json};
use std::borrow::Cow;

pub fn schema() -> Json {
    Json::from(schemars::schema_for!(Scene))
}

pub fn property_schema() -> Json {
    Json::from(schemars::schema_for!(Property))
}

pub fn easings() -> Vec<String> {
    let families = FAMILIES.iter().flat_map(|f| ["in_", "out_", "in_out_"].map(|p| format!("{p}{f}")));
    ["linear", "step", "hold"].into_iter().map(String::from).chain(families).collect()
}

impl JsonSchema for Ease {
    fn schema_name() -> Cow<'static, str> {
        "Ease".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        let bezier = json!({"type": "array", "items": {"type": "number"}, "minItems": 4, "maxItems": 4});
        json_schema!({"anyOf": [{"type": "string", "enum": easings()}, bezier]})
    }
}

impl JsonSchema for Text {
    fn schema_name() -> Cow<'static, str> {
        "Text".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        let languages = json!({"type": "object", "additionalProperties": {"type": "string"}});
        json_schema!({"anyOf": [{"type": "string"}, languages]})
    }
}

impl<T: JsonSchema> JsonSchema for Key<T> {
    fn schema_name() -> Cow<'static, str> {
        format!("Key_for_{}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let (value, ease) = (generator.subschema_for::<T>(), generator.subschema_for::<Ease>());
        let curve = json!({"anyOf": [ease, generator.subschema_for::<Handles>()]});
        let items = json!([{"type": "number", "minimum": 0}, value, curve]);
        json_schema!({"type": "array", "prefixItems": items, "minItems": 2, "maxItems": 3, "x-key": true})
    }
}

pub(crate) fn fit(_: &mut SchemaGenerator) -> Schema {
    json_schema!({"type": "string", "enum": ["fill", "fit", "stretch", "center"], "default": "fill"})
}

pub(crate) fn fps(_: &mut SchemaGenerator) -> Schema {
    let number = json!({"type": "integer", "minimum": 0, "maximum": 240});
    json_schema!({"anyOf": [number, {"type": "string", "const": "display"}], "default": 60})
}
