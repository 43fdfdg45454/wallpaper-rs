use crate::cli::{ITEM, JSON, Out, Page};
use clap::Subcommand;

mod act;
mod shown;

pub use act::run;
pub use shown::history;
pub use shown::page;
pub use shown::playlists;
pub use shown::rules;

const PL: &str = "Playlist uid or name";

#[derive(Subcommand, Debug)]
pub enum Playlist {
    #[command(about = "List the playlists")]
    Ls {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Show a playlist and a page of its items")]
    Show {
        #[arg(value_name = "PL", help = PL)]
        playlist: String,
        #[command(flatten)]
        page: Page,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Create a playlist and print its uid")]
    New {
        name: String,
        #[arg(long, default_value = "", help = "Search query of a smart playlist; manual when omitted")]
        query: String,
    },
    #[command(about = "Rename a playlist")]
    Rename {
        #[arg(value_name = "PL", help = PL)]
        playlist: String,
        name: String,
    },
    #[command(about = "Change the query of a smart playlist; \"\" makes it manual")]
    Query {
        #[arg(value_name = "PL", help = PL)]
        playlist: String,
        query: String,
    },
    #[command(about = "Delete a playlist")]
    Rm {
        #[arg(value_name = "PL", help = PL)]
        playlist: String,
    },
    #[command(about = "Append items to a manual playlist")]
    Add {
        #[arg(value_name = "PL", help = PL)]
        playlist: String,
        #[arg(required = true, value_name = "ITEM", help = ITEM)]
        items: Vec<String>,
    },
    #[command(about = "Remove every occurrence of items from a manual playlist")]
    Remove {
        #[arg(value_name = "PL", help = PL)]
        playlist: String,
        #[arg(required = true, value_name = "ITEM", help = ITEM)]
        items: Vec<String>,
    },
    #[command(about = "Remove the entries at 0-based positions of a manual playlist")]
    RemoveAt {
        #[arg(value_name = "PL", help = PL)]
        playlist: String,
        #[arg(required = true, value_name = "INDEX")]
        indexes: Vec<u32>,
    },
    #[command(about = "Move an entry of a manual playlist from one 0-based position to another")]
    Move {
        #[arg(value_name = "PL", help = PL)]
        playlist: String,
        from: u32,
        to: u32,
    },
    #[command(about = "Print the rotation rules, one rule, or change a rule")]
    Rule {
        #[arg(value_name = "PL", help = PL)]
        playlist: String,
        key: Option<String>,
        value: Option<String>,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Show a playlist on the outputs")]
    Start {
        #[arg(value_name = "PL", help = PL)]
        playlist: String,
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "Stop the playlist of the outputs, keeping the item shown now")]
    Stop(Out),
    #[command(about = "Print the items an output showed, newest first")]
    History {
        #[arg(short, long, help = "Output name such as DP-1")]
        output: String,
        #[arg(long, default_value_t = 20)]
        limit: u32,
        #[arg(long, help = JSON)]
        json: bool,
    },
}
