use crate::library1::Library;
use crate::playlists1::Playlists;
use crate::profiles1::Profiles;
use crate::providers1::Providers;
use crate::repos1::Repos;
use crate::service::Service;
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
    let interfaces: [(&str, Box<dyn Interface>); 6] = [
        (dbus::INTERFACE, Box::new(Service(Weak::new()))),
        (dbus::LIBRARY, Box::new(Library(Weak::new()))),
        (dbus::PLAYLISTS, Box::new(Playlists(Weak::new()))),
        (dbus::PROFILES, Box::new(Profiles(Weak::new()))),
        (dbus::PROVIDERS, Box::new(Providers(Weak::new()))),
        (dbus::REPOS, Box::new(Repos(Weak::new()))),
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
