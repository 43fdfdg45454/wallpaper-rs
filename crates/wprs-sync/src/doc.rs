use crate::Hlc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub type View = BTreeMap<String, Value>;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Doc {
    #[serde(rename = "f", default)]
    pub fields: BTreeMap<String, (Value, Hlc)>,
    #[serde(rename = "g", default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<Hlc>,
}

impl Doc {
    pub fn cleared() -> Doc {
        Doc { fields: BTreeMap::new(), gone: Some(Hlc::default()) }
    }

    pub fn clock(&self) -> Hlc {
        self.fields.values().map(|(_, clock)| clock).chain(&self.gone).max().cloned().unwrap_or_default()
    }

    pub fn view(&self) -> Option<View> {
        let latest = self.fields.values().map(|(_, clock)| clock).max();
        if self.gone.as_ref().is_some_and(|gone| latest.is_none_or(|l| gone > l)) {
            return None;
        }
        let set = self.fields.iter().filter(|(_, (value, _))| !value.is_null());
        Some(set.map(|(key, (value, _))| (key.clone(), value.clone())).collect())
    }

    pub fn merge(&self, theirs: &Doc) -> Doc {
        let mut fields = self.fields.clone();
        for (key, field) in &theirs.fields {
            if fields.get(key).is_none_or(|mine| field.1 >= mine.1) {
                fields.insert(key.clone(), field.clone());
            }
        }
        Doc { fields, gone: self.gone.clone().max(theirs.gone.clone()) }
    }

    pub fn stamp(base: Option<&Doc>, local: Option<&View>, weak: fn(&str) -> bool, now: &Hlc) -> Option<Doc> {
        let seen = base.and_then(Doc::view);
        if seen.as_ref() == local {
            return None;
        }
        let Some(local) = local else {
            let mut doc = base?.clone();
            doc.gone = Some(now.clone());
            return Some(doc);
        };
        let mut doc = base.cloned().unwrap_or_default();
        let keys: BTreeSet<String> = local.keys().chain(doc.fields.keys()).cloned().collect();
        for key in keys {
            let value = local.get(&key).cloned().unwrap_or(Value::Null);
            if seen.as_ref().and_then(|s| s.get(&key)).unwrap_or(&Value::Null) == &value {
                continue;
            }
            let clock = if base.is_none() && weak(&key) { Hlc::default() } else { now.clone() };
            doc.fields.insert(key, (value, clock));
        }
        Some(doc)
    }
}
