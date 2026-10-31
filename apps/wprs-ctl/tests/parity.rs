use clap::CommandFactory;
use std::collections::HashSet;
use wprs_core::edit::{ACTIONS, Action};
use wprs_ctl::{CALLS, Cli};

const NOT_IN_CLI: &[&str] = &["Library1.RequestThumbnails", "Inputs1.Touch", "Sync1.Approve"];

fn documented(text: &str) -> Vec<(String, Vec<String>)> {
    let mut interface = None;
    let mut members = Vec::new();
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let name = heading.strip_prefix("`io.github.wallpaper_rs.").and_then(|h| h.strip_suffix('`'));
            interface = name.map(String::from);
        } else if let (Some(interface), Some(member)) = (&interface, line.strip_prefix("### `")) {
            let (name, args) = member.split_once('(').expect("a member heading lists its arguments");
            let args = args.split(')').next().unwrap_or_default().split(", ").filter(|a| !a.is_empty());
            let args = args.map(|a| a.rsplit(' ').next().unwrap_or(a).to_string()).collect();
            members.push((format!("{interface}.{name}"), args));
        }
    }
    members
}

fn subcommand(path: &str) -> clap::Command {
    let find = |command: clap::Command, name| command.find_subcommand(name).cloned();
    path.split(' ').try_fold(Cli::command(), find).unwrap_or_else(|| panic!("no command {path}"))
}

fn supplies(command: &clap::Command, token: &str) -> bool {
    token.starts_with('=')
        || token == "prompt"
        || command.get_arguments().any(|arg| match token.strip_prefix("--") {
            Some(long) => arg.get_long() == Some(long),
            None => {
                let named =
                    arg.get_value_names().is_some_and(|names| names.iter().any(|n| n.as_str() == token));
                arg.is_positional() && (named || arg.get_id().as_str().to_uppercase() == token)
            }
        })
}

#[test]
fn every_call_is_supplied_by_its_command() {
    let mut seen = HashSet::new();
    for (interface, method, path, tokens) in CALLS {
        assert!(seen.insert((interface, method)), "{interface}.{method} is listed twice");
        let command = subcommand(path);
        for token in *tokens {
            assert!(supplies(&command, token), "{interface}.{method}: wprs-ctl {path} has no {token}");
        }
    }
}

#[test]
fn every_documented_method_has_a_command() {
    let text =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/specs/dbus.md")).unwrap();
    let members = documented(&text);
    assert!(!members.is_empty(), "docs/specs/dbus.md has no interface member headings");
    for (member, args) in members.iter().filter(|(member, _)| !NOT_IN_CLI.contains(&member.as_str())) {
        let call = CALLS.iter().find(|(interface, method, ..)| format!("{interface}.{method}") == *member);
        let (.., tokens) = call.unwrap_or_else(|| panic!("{member} has no wprs-ctl command"));
        assert_eq!(tokens.len(), args.len(), "{member} documents the arguments {args:?}");
    }
    for (interface, method, ..) in CALLS {
        let member = format!("{interface}.{method}");
        assert!(members.iter().any(|(m, _)| *m == member), "{member} is not documented");
    }
}

fn expected(value: serde_json::Value) -> Vec<String> {
    let refused = serde_json::from_value::<Action>(value).unwrap_err().to_string();
    refused.split('`').skip(3).step_by(2).map(String::from).collect()
}

#[test]
fn every_editor_action_has_an_edit_command_with_the_same_arguments() {
    assert_eq!(expected(serde_json::json!({"": {}})), ACTIONS);
    for name in ACTIONS {
        let command = subcommand(&format!("edit {name}"));
        for field in expected(serde_json::json!({name: {"": null}})) {
            assert!(
                command.get_arguments().any(|a| a.get_id() == field.as_str()),
                "edit {name} lacks {field}"
            );
        }
        for id in
            command.get_arguments().map(|a| a.get_id().as_str()).filter(|id| !["path", "help"].contains(id))
        {
            let refused = serde_json::from_value::<Action>(serde_json::json!({name: {id: null}})).err();
            let message = refused.map(|e| e.to_string()).unwrap_or_default();
            assert!(!message.contains("unknown field"), "wprs-ctl edit {name} {id}: {message}");
        }
    }
}
