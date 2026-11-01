use clap::{ArgAction, Subcommand};
use std::path::PathBuf;

pub(crate) const DIR: &str = "Scene folder: manifest.json, scene.json and the assets";
pub(crate) const POINTER: &str =
    "JSON Pointer into the document, such as /scene/layers/0/opacity or /manifest/title";
const VALUE: &str = "JSON value; text that is not JSON is taken as a string";
pub(crate) const ID: &str = "Layer id";

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
    #[command(flatten)]
    Layers(crate::layering::Layering),
    #[command(about = "Copy a picture, video, sound, font or .cube table into the folder as an asset")]
    Import {
        #[arg(help = DIR)]
        dir: PathBuf,
        file: PathBuf,
        #[arg(long, help = "Asset id; made from the file name when omitted")]
        id: Option<String>,
    },
    #[command(
        about = "Copy an effect descriptor and its WGSL shader into the folder as an effect of the package"
    )]
    ImportEffect {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "The descriptor, a .json file")]
        descriptor: PathBuf,
        #[arg(long, help = "The shader; the .wgsl file next to the descriptor when omitted")]
        wgsl: Option<PathBuf>,
    },
    #[command(
        about = "Copy a JavaScript module into the scripts of the folder and declare the events and parts it uses"
    )]
    ImportScript {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "The module, a .js file")]
        file: PathBuf,
    },
    #[command(
        about = "Paint a stroke into the picture of a mask parameter of an effect, on one channel or on all colors, kept as a new linear picture in masks/ so that undo returns to the previous one"
    )]
    PaintMask {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "JSON Pointer to the parameter, such as /scene/layers/0/effects/0/params/regions")]
        pointer: String,
        #[arg(help = "The stroke as fractions of the layer, X,Y;X,Y;...")]
        points: String,
        #[arg(long, value_parser = ["r", "g", "b", "a"], help = "Channel to paint; all colors when omitted")]
        channel: Option<String>,
        #[arg(long, default_value_t = 0.05, help = "Radius of the brush, a fraction of the shorter side")]
        radius: f32,
        #[arg(long, default_value_t = 0.5, help = "Hardness of the edge of the brush, from 0 to 1")]
        hardness: f32,
        #[arg(long, default_value_t = 1.0, help = "Value painted, from 0 (erases) to 1")]
        value: f32,
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
    #[command(flatten)]
    Properties(crate::userprops::UserProps),
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
    #[command(flatten)]
    Folder(crate::folder::Folder),
    #[command(flatten)]
    Rigging(crate::rigging::Rigging),
    #[command(flatten)]
    Clipping(crate::clipping::Clipping),
}
