use clap::{ArgAction, Subcommand};
use std::path::PathBuf;

const DIR: &str = "Scene folder: manifest.json, scene.json and the assets";
const POINTER: &str = "JSON Pointer into the document, such as /scene/layers/0/opacity or /manifest/title";
const VALUE: &str = "JSON value; text that is not JSON is taken as a string";
const ID: &str = "Layer id";

#[derive(Subcommand, Debug)]
pub enum Author {
    #[command(about = "Create a scene folder from a starting point: empty, particles, parallax or clock")]
    New {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, default_value = "empty", help = "Starting point")]
        from: String,
        #[arg(long, help = "Package id; org.example.<folder name> when omitted")]
        id: Option<String>,
        #[arg(long, help = "Title; the folder name when omitted")]
        title: Option<String>,
        #[arg(long, help = "Language of the title, such as es or pt-BR; the desktop language when omitted")]
        locale: Option<String>,
    },
    #[command(
        about = "Describe a scene folder, or first extract a .wpak scene package into it as an unsigned copy with the id <id>.copy"
    )]
    Open {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, value_name = "FILE", help = "Scene package to extract into the new folder")]
        from: Option<PathBuf>,
        #[arg(long, help = "Print the description as JSON")]
        json: bool,
    },
    #[command(about = "Print the document, or the value at a JSON Pointer, as JSON")]
    Show {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = POINTER)]
        pointer: Option<String>,
    },
    #[command(about = "Print the fields the inspector shows for a JSON Pointer, as JSON")]
    Fields {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = POINTER)]
        pointer: String,
    },
    #[command(about = "Set the value at a JSON Pointer, creating the objects on the way")]
    Set {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = POINTER)]
        pointer: String,
        #[arg(allow_hyphen_values = true, help = VALUE)]
        value: String,
    },
    #[command(about = "Insert a value into a list (index or -) or an object, as JSON Patch add does")]
    Add {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = POINTER)]
        pointer: String,
        #[arg(allow_hyphen_values = true, help = VALUE)]
        value: String,
    },
    #[command(about = "Remove the value at a JSON Pointer")]
    Remove {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = POINTER)]
        pointer: String,
    },
    #[command(about = "Move the value at one JSON Pointer to another")]
    Move {
        #[arg(help = DIR)]
        dir: PathBuf,
        from: String,
        to: String,
    },
    #[command(
        about = "Add a layer: image, video, text, shape, solid, spectrum, particles, sound, group or adjustment"
    )]
    AddLayer {
        #[arg(help = DIR)]
        dir: PathBuf,
        kind: String,
        #[arg(long, help = "Id of the new layer; made from the kind when omitted")]
        id: Option<String>,
        #[arg(long, help = "Asset of an image, video or sound layer, or system of a particles layer")]
        source: Option<String>,
        #[arg(long, help = "Group to add the layer to")]
        into: Option<String>,
        #[arg(long, help = "Position among its siblings, 0 at the bottom; on top when omitted")]
        at: Option<usize>,
    },
    #[command(about = "Remove a layer")]
    RemoveLayer {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
    },
    #[command(about = "Copy a layer, with new ids, just above it")]
    DuplicateLayer {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
    },
    #[command(about = "Move a layer to a position, 0 at the bottom, among the top layers or in a group")]
    MoveLayer {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
        to: usize,
        #[arg(long, help = "Group to move the layer into")]
        into: Option<String>,
    },
    #[command(about = "Put layers of the same group into a new group")]
    Group {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(required = true)]
        ids: Vec<String>,
        #[arg(long, help = "Id of the new group")]
        id: Option<String>,
    },
    #[command(about = "Replace a group with its layers")]
    Ungroup {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
    },
    #[command(about = "Hide a layer, or mute a sound layer; --hidden false shows it again")]
    Hide {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        hidden: bool,
    },
    #[command(about = "Lock a layer against changes on the canvas; --locked false unlocks it")]
    Lock {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        locked: bool,
    },
    #[command(about = "Show only this layer in previews, or every layer again without an id")]
    Solo {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: Option<String>,
    },
    #[command(about = "Add or replace a keyframe of an animatable value, or remove it with --remove")]
    Keyframe {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = POINTER)]
        pointer: String,
        #[arg(help = "Seconds on the timeline, or on the scene clock without one")]
        time: f64,
        #[arg(allow_hyphen_values = true, help = "JSON value; the current value when omitted")]
        value: Option<String>,
        #[arg(long, help = "Easing towards the next keyframe, such as in_out_sine, or [x1,y1,x2,y2]")]
        ease: Option<String>,
        #[arg(long, help = "Timeline that drives the keyframes")]
        timeline: Option<String>,
        #[arg(long)]
        remove: bool,
    },
    #[command(about = "Copy a picture, video, sound, font or .cube table into the folder as an asset")]
    Import {
        #[arg(help = DIR)]
        dir: PathBuf,
        file: PathBuf,
        #[arg(long, help = "Asset id; made from the file name when omitted")]
        id: Option<String>,
    },
    #[command(
        about = "Set the text of a title, description, change notes, label or option in one language; an empty language sets the plain text"
    )]
    Translate {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "JSON Pointer to the text, such as /manifest/title")]
        pointer: String,
        #[arg(help = "Language such as en or pt-BR")]
        language: String,
        #[arg(
            allow_hyphen_values = true,
            required_unless_present_any = ["remove", "add"],
            help = "Text in that language; an empty text removes it"
        )]
        text: Option<String>,
        #[arg(long, conflicts_with_all = ["text", "add"], help = "Remove the text in that language")]
        remove: bool,
        #[arg(long, help = "Only add a language the text lacks, starting from the default language")]
        add: bool,
    },
    #[command(
        about = "Copy a picture into assets/ as the thumbnail of the package, made smaller when it is over 1024 pixels or 1 MiB, or with --animated as its animated preview"
    )]
    Thumbnail {
        #[arg(help = DIR)]
        dir: PathBuf,
        file: PathBuf,
        #[arg(long, help = "A gif or webp of at most 640 pixels a side, 15 s and 8 MiB")]
        animated: bool,
    },
    #[command(
        about = "Move a first-version manifest to wallpaper-rs.wpak/2, as wprs-ctl pack upgrade does; what is not given stays to be set"
    )]
    Upgrade {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(
            long,
            value_parser = ["general", "mature"],
            help = "Who the package suits; the origin is then original unless --derived-from names a work"
        )]
        rating: Option<String>,
        #[arg(long = "reason", value_parser = ["sexual", "violence", "gore", "language"], help = "Why it is mature")]
        reasons: Vec<String>,
        #[arg(long, action = ArgAction::Set, help = "true when it flashes or shows fast patterns")]
        photosensitive: Option<bool>,
        #[arg(long = "derived-from", value_name = "AUTHOR", help = "Author of a work it is based on")]
        derived_from: Vec<String>,
    },
    #[command(about = "Delete the files in assets/ that the scene does not use")]
    Prune {
        #[arg(help = DIR)]
        dir: PathBuf,
    },
    #[command(about = "Undo the last change")]
    Undo {
        #[arg(help = DIR)]
        dir: PathBuf,
    },
    #[command(about = "Redo the last undone change")]
    Redo {
        #[arg(help = DIR)]
        dir: PathBuf,
    },
    #[command(about = "List the changes that undo and redo walk through")]
    History {
        #[arg(help = DIR)]
        dir: PathBuf,
    },
    #[command(about = "Restore the unsaved changes the app kept for a folder, or drop them with --discard")]
    Recover {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long)]
        discard: bool,
    },
    #[command(about = "Check the scene and its files; exits with 1 when something is wrong")]
    Validate {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, help = "Print one JSON object per problem")]
        json: bool,
    },
    #[command(about = "Build the .wpak package, signed with --key, as wprs-ctl pack build does")]
    Build {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(short, long, help = "Package file; <id>-<version>.wpak next to the scene folder when omitted")]
        out: Option<PathBuf>,
        #[arg(long, help = "Secret key file of the author")]
        key: Option<PathBuf>,
        #[arg(long, help = "Print the result as JSON")]
        json: bool,
    },
    #[command(about = "Show the scene of the folder on the desktop for 60 seconds without applying it")]
    Preview {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(short, long, default_value = "", help = crate::cli::OUTPUT)]
        output: String,
        #[arg(long, default_value_t = -1.0, allow_hyphen_values = true, help = "Hold this second; plays when negative")]
        time: f64,
        #[arg(long, help = "End the preview")]
        stop: bool,
    },
    #[command(about = "Build the scene into the library and show it on an output")]
    Apply {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(short, long, default_value = "", help = crate::cli::OUTPUT)]
        output: String,
    },
    #[command(
        about = "Print the JSON Schema of scenes, of user properties with --property or of manifests with --manifest"
    )]
    Schema {
        #[arg(long)]
        property: bool,
        #[arg(long, conflicts_with = "property")]
        manifest: bool,
    },
}
