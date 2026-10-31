use serde_json::{Value, json};

const OWN: &str = "wpak: wpak-user: data: blob:";

pub fn csp(network: &[String]) -> String {
    wprs_format::page::csp(OWN, "wpak:", network)
}

fn escaped(origin: &str) -> String {
    origin.chars().flat_map(|c| if c.is_ascii_alphanumeric() { vec![c] } else { vec!['\\', c] }).collect()
}

pub fn filter(network: &[String]) -> String {
    let allow = |pattern: String| json!({"trigger": {"url-filter": pattern}, "action": {"type": "ignore-previous-rules"}});
    let own = ["^wpak:", "^wpak-user:", "^data:", "^blob:"].map(|p| allow(p.into()));
    let hosts = network.iter().map(|o| allow(format!("^{}/", escaped(o))));
    let block = json!({"trigger": {"url-filter": ".*"}, "action": {"type": "block"}});
    Value::Array(std::iter::once(block).chain(own).chain(hosts).collect()).to_string()
}

pub fn languages(real: &[String], hidden: bool) -> Vec<String> {
    let tag = |l: &String| l.split(['.', '@']).next().unwrap_or_default().replace('_', "-");
    let mut tags: Vec<String> =
        real.iter().map(tag).filter(|l| !l.is_empty() && l != "C" && l != "POSIX").collect();
    tags.dedup();
    match tags.is_empty() || hidden {
        true => vec!["en-US".into()],
        false => tags,
    }
}

pub use wprs_format::page::mime;
