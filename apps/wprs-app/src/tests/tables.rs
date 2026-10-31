use super::qml_files;
use std::collections::HashMap;

type Tables = HashMap<String, HashMap<String, String>>;

pub fn tables() -> Tables {
    let mut out = Tables::new();
    let more = [
        "Labels.qml",
        "SceneValues.qml",
        "SceneEffects.qml",
        "SceneTerms.qml",
        "SceneNames.qml",
        "SceneLabels.qml",
    ];
    for (file, text) in
        qml_files().into_iter().filter(|(n, _)| n.ends_with("Words.qml") || more.contains(&n.as_str()))
    {
        for block in text.split("readonly property var ").skip(1) {
            let Some((name, body)) = block.split_once(": ({").filter(|(n, _)| !n.contains(' ')) else {
                continue;
            };
            let rows = body.split("})").next().unwrap().lines().filter_map(|line| {
                let (key, text) = line.trim().split_once("\": App.tr(\"")?;
                Some((
                    key.trim_start_matches('"').into(),
                    text.trim_end_matches(',').strip_suffix("\")")?.into(),
                ))
            });
            let name = if name == "early" { "words" } else { name };
            out.entry(format!("{}.{name}", &file[..1])).or_default().extend(rows);
        }
    }
    out
}

pub struct Words(pub Tables);

impl Words {
    pub fn get(&self, table: &str, key: &str) -> Option<String> {
        self.0.get(table)?.get(key).cloned()
    }

    pub fn label(&self, key: &str, parent: &str) -> Option<String> {
        let keys = [format!("{parent}.{key}"), key.to_owned()];
        keys.iter().find_map(|k| self.get("S.words", k).or_else(|| self.get("M.words", k)))
    }

    pub fn value(&self, text: &str) -> Option<String> {
        let tables = ["S.values", "M.values", "S.kinds", "S.effects"];
        let found = tables.iter().find_map(|t| self.get(t, text));
        found.or_else(|| self.label(text, "")).or_else(|| self.easing(text))
    }

    pub fn easing(&self, name: &str) -> Option<String> {
        let ways = [("in_out_", "Ease in and out (%1)"), ("in_", "Ease in (%1)"), ("out_", "Ease out (%1)")];
        let (curve, text) = ways.iter().find_map(|(w, t)| Some((name.strip_prefix(w)?, *t)))?;
        self.get("S.curves", curve).map(|_| text.to_owned())
    }
}
