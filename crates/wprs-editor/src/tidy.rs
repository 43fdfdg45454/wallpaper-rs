use serde_json::{Value, json};
use std::collections::BTreeSet;

const LISTS: [&str; 6] = ["clips", "animations", "ik", "physics", "rules", "hit"];

pub(crate) struct Tree<'a> {
    pub names: &'a [String],
    pub parents: &'a [Option<usize>],
}

impl Tree<'_> {
    fn up(&self, name: &str) -> Option<String> {
        let n = self.names.iter().position(|x| x == name)?;
        self.parents[n].map(|p| self.names[p].clone())
    }

    fn hangs(&self, chain: &[Value]) -> bool {
        chain.windows(2).all(|w| w[1].as_str().and_then(|b| self.up(b)).is_some_and(|p| w[0] == p.as_str()))
    }

    fn looped(&self, rules: &[Value], i: usize) -> bool {
        let name = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        let bone = name(&rules[i]["bone"]);
        let (mut stack, mut seen) = (vec![name(&rules[i]["parent"])], BTreeSet::new());
        while let Some(at) = stack.pop() {
            if at == bone {
                return true;
            }
            if seen.insert(at.clone()) {
                stack.extend(self.up(&at));
                stack.extend(rules.iter().filter(|r| r["bone"] == at.as_str()).map(|r| name(&r["parent"])));
            }
        }
        false
    }
}

fn list<'a>(l: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
    if !l[key].is_array() {
        l[key] = json!([]);
    }
    l[key].as_array_mut().expect("a list")
}

fn clips(l: &mut Value, gone: &str, keep: Option<&str>) {
    for c in list(l, "clips") {
        let Some(bones) = c.get_mut("bones").and_then(Value::as_object_mut) else { continue };
        if bones.remove(gone).is_none() {
            continue;
        }
        let tracked =
            ["bones", "shapes", "order"].iter().any(|k| c[*k].as_object().is_some_and(|m| !m.is_empty()));
        if let Some(b) = keep.filter(|_| !tracked && c.get("animation").is_none()) {
            c["bones"] = json!({ b: { "rotation": [[c["from"].as_f64().unwrap_or(0.0), 0]] } });
        }
    }
}

pub(crate) fn layer(l: &mut Value, before: &Tree, after: &Tree, gone: Option<&str>, keep: Option<&str>) {
    let old = l.clone();
    let is = |v: &Value| gone.is_some_and(|g| v == g);
    if let Some(g) = gone {
        clips(l, g, keep);
        list(l, "animations").retain_mut(|a| {
            let Some(mask) = a.get_mut("bones").and_then(Value::as_array_mut) else { return true };
            let had = mask.len();
            mask.retain(|b| !is(b));
            mask.len() == had || !mask.is_empty()
        });
        list(l, "physics").retain(|p| !is(&p["bone"]));
        list(l, "hit").retain(|b| !is(b));
    }
    list(l, "ik").retain_mut(|c| {
        let had = c["chain"].as_array().map_or(0, Vec::len);
        let linked = before.hangs(c["chain"].as_array().map_or(&[], Vec::as_slice));
        if let Some(g) = gone {
            c["chain"].as_array_mut().into_iter().for_each(|ch| ch.retain(|b| b != g));
            c["limits"].as_object_mut().into_iter().for_each(|m| drop(m.remove(g)));
            if c["target"] == format!("bone:{g}").as_str() {
                return false;
            }
        }
        let chain = c["chain"].as_array().map_or(&[][..], Vec::as_slice);
        (chain.len() == had || chain.len() >= 2) && (!linked || after.hangs(chain))
    });
    let rules = list(l, "rules");
    rules.retain(|r| !is(&r["bone"]) && !is(&r["parent"]));
    while let Some(i) = (0..rules.len()).find(|i| after.looped(rules, *i) && !before.looped(rules, *i)) {
        rules.remove(i);
    }
    let emptied = |k: &str, v: &Value| v.as_array().is_some_and(Vec::is_empty) && old[k] != json!([]);
    if let Some(m) = l.as_object_mut() {
        m.retain(|k, v| !LISTS.contains(&k.as_str()) || !emptied(k, v));
    }
}
