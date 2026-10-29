use crate::inputs1::Inputs;
use crate::library1::Library;
use crate::playlists1::Playlists;
use crate::profiles1::Profiles;
use crate::providers1::Providers;
use crate::repos1::Repos;
use crate::service::Service;
use crate::sync1::Sync;
use std::sync::Weak;
use wprs_core::dbus;
use zbus::object_server::Interface;

fn members(xml: &str) -> Vec<String> {
    let doc = roxmltree::Document::parse(xml).unwrap();
    let args = |node: roxmltree::Node<'_, '_>, direction: Option<&str>| {
        let args = node.children().filter(|a| a.has_tag_name("arg"));
        let args = args.filter(|a| direction.is_none_or(|d| a.attribute("direction").unwrap_or("in") == d));
        let args = args.map(|a| format!("{} {}", a.attribute("type").unwrap(), a.attribute("name").unwrap()));
        args.collect::<Vec<_>>().join(", ")
    };
    let mut found = Vec::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        let name = node.attribute("name").unwrap_or_default();
        match node.tag_name().name() {
            "method" => found.push(match args(node, Some("out")).as_str() {
                "" => format!("`{name}({})`", args(node, Some("in"))),
                out => format!("`{name}({}) → ({out})`", args(node, Some("in"))),
            }),
            "signal" => found.push(format!("Signal `{name}({})`", args(node, None))),
            "property" => found.push(format!("Property `{name}` (`{}`)", node.attribute("type").unwrap())),
            _ => {}
        }
    }
    found
}

fn introspect(interface: &dyn Interface) -> String {
    let mut xml = String::new();
    interface.introspect_to_writer(&mut xml, 0);
    format!("<node>{xml}</node>")
}

#[test]
fn every_documented_member_exists_and_the_reverse() {
    let spec = include_str!("../../../docs/specs/dbus.md");
    let interfaces: [(&str, Box<dyn Interface>); 8] = [
        (dbus::INTERFACE, Box::new(Service(Weak::new()))),
        (dbus::LIBRARY, Box::new(Library(Weak::new()))),
        (dbus::PLAYLISTS, Box::new(Playlists(Weak::new()))),
        (dbus::PROFILES, Box::new(Profiles(Weak::new()))),
        (dbus::PROVIDERS, Box::new(Providers(Weak::new()))),
        (dbus::REPOS, Box::new(Repos(Weak::new()))),
        (dbus::INPUTS, Box::new(Inputs(Weak::new()))),
        (dbus::SYNC, Box::new(Sync(Weak::new()))),
    ];
    let mut sections = spec.split("\n## ").skip(1);
    for (name, interface) in interfaces {
        let xml = introspect(interface.as_ref());
        assert!(xml.contains(&format!("<interface name=\"{name}\">")), "{xml}");
        let section = sections.find(|s| s.starts_with(&format!("`{name}`")));
        let section = section.unwrap_or_else(|| panic!("{name} is not documented in this order"));
        let mut documented: Vec<String> =
            section.lines().filter_map(|l| l.strip_prefix("### ")).map(String::from).collect();
        let mut served = members(&xml);
        documented.sort();
        served.sort();
        assert_eq!(documented, served, "{name}");
    }
}

#[test]
fn qml_signal_handlers_take_every_argument_or_none() {
    let served = [
        introspect(&Service(Weak::new())),
        introspect(&Library(Weak::new())),
        introspect(&Repos(Weak::new())),
    ];
    let signals: Vec<(String, usize)> = (served.iter().flat_map(|xml| members(xml)))
        .filter_map(|m| {
            let (name, args) = m.strip_prefix("Signal `")?.split_once('(')?;
            Some((name.to_owned(), args.trim_end_matches(")`").split(", ").filter(|a| !a.is_empty()).count()))
        })
        .collect();
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut checked = 0;
    for dir in ["integrations/plasma-tray", "integrations/plasma-wallpaper", "apps/wprs-app/qml"] {
        let files = walk(&std::path::Path::new(root).join(dir));
        for text in files.iter().filter_map(|f| std::fs::read_to_string(f).ok()) {
            for handler in text.split("function dbus").skip(1) {
                let (name, rest) = handler.split_once('(').unwrap();
                let count =
                    rest.split(')').next().unwrap().split(',').filter(|a| !a.trim().is_empty()).count();
                if let Some((_, wanted)) = signals.iter().find(|(signal, _)| signal == name) {
                    assert!(count == 0 || count == *wanted, "dbus{name} takes {count} of {wanted} arguments");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked >= 4, "only {checked} handlers found");
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path());
    entries
        .flat_map(|p| if p.is_dir() { walk(&p) } else { vec![p] })
        .filter(|p| p.extension().is_some_and(|e| e == "qml"))
        .collect()
}
