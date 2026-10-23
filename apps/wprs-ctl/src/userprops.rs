use crate::author::DIR;
use clap::Subcommand;
use std::path::PathBuf;
use wprs_editor::Action;

const PROPERTY: &str = "Id of a user property";
const KINDS: &str = "slider, bool, color, combo, text, file, folder, button, group, label or divider";

#[derive(Subcommand, Debug)]
pub enum UserProps {
    #[command(about = "Add a user property of one of the kinds")]
    AddProperty {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = KINDS)]
        kind: String,
        #[arg(long, help = "Id of the new property; made from the kind when omitted")]
        id: Option<String>,
        #[arg(long, help = "Group to add the property to")]
        group: Option<String>,
        #[arg(long, help = "Label in the default language; the id shows when omitted")]
        label: Option<String>,
    },
    #[command(about = "Move a user property among the properties of its group, or into another group")]
    MoveProperty {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = PROPERTY)]
        id: String,
        #[arg(help = "Position among the properties of its group, 0 first")]
        to: usize,
        #[arg(long, help = "Group to move it into; an empty one takes it out of its group")]
        group: Option<String>,
    },
    #[command(
        about = "Rename a user property together with the bindings, triggers, groups, conditions and presets that name it; scripts are not rewritten"
    )]
    RenameProperty {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = PROPERTY)]
        id: String,
        #[arg(help = "New id")]
        to: String,
    },
    #[command(
        about = "Remove a user property, its values in presets and conditions and the group of its members; refused while bindings, triggers or expressions name it"
    )]
    RemoveProperty {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = PROPERTY)]
        id: String,
    },
    #[command(about = "Set when a user property is shown, or with --enabled when it can be changed")]
    Condition {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = PROPERTY)]
        id: String,
        #[arg(
            allow_hyphen_values = true,
            help = "A JSON map of values or an expression such as \"mood == 2 || speed > 1.5\"; an empty one removes it"
        )]
        text: String,
        #[arg(long, help = "Set enabled_if instead of visible_if")]
        enabled: bool,
    },
    #[command(about = "Add a preset of the package; set DIR /manifest/presets/N/values/ID VALUE fills it")]
    AddPreset {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, help = "Id of the preset; preset_<n> when omitted")]
        id: Option<String>,
        #[arg(long, help = "Label in the default language")]
        label: Option<String>,
    },
}

pub fn action(command: UserProps) -> (PathBuf, Action) {
    match command {
        UserProps::AddProperty { dir, kind, id, group, label } => {
            (dir, Action::AddProperty { kind, id, group, label })
        }
        UserProps::MoveProperty { dir, id, to, group } => (dir, Action::MoveProperty { id, to, group }),
        UserProps::RenameProperty { dir, id, to } => (dir, Action::RenameProperty { id, to }),
        UserProps::RemoveProperty { dir, id } => (dir, Action::RemoveProperty { id }),
        UserProps::Condition { dir, id, text, enabled } => (dir, Action::Condition { id, enabled, text }),
        UserProps::AddPreset { dir, id, label } => (dir, Action::AddPreset { id, label }),
    }
}
