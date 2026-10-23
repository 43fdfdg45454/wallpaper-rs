use crate::cli::{FIT, ITEM, JSON, OUTPUT, Page, Wait};
use crate::daemon::job;
use crate::input::{one, path_arg, resolve, strs};
use crate::items::{details, search, sources, tags};
use crate::show;
use anyhow::Result;
use clap::{ArgGroup, Subcommand};
use futures_util::TryFutureExt;
use wprs_client::Client;
use wprs_core::Fit;

#[derive(Subcommand, Debug)]
pub enum Lib {
    #[command(about = "Add a folder and scan it; prints the source uid, then the job id")]
    Add {
        dir: String,
        #[arg(long, help = "Leave out subfolders")]
        no_recursive: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "List the library folders")]
    Sources {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Scan one folder, or every folder when omitted")]
    Scan {
        source: Option<String>,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Remove a folder and its items from the library")]
    Unsource { source: String },
    #[command(about = "Import image, video or package files")]
    Import {
        #[arg(required = true, value_name = "FILE")]
        files: Vec<String>,
        #[arg(long, help = "Copy the files into the library first")]
        copy: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Remove items from the library")]
    Rm {
        #[arg(required = true, value_name = "ITEM", help = ITEM)]
        items: Vec<String>,
        #[arg(long, help = "Also delete the imported copies")]
        delete_files: bool,
    },
    #[command(about = "Search the library; see wprs-ctl schema for the query keys")]
    Search {
        #[arg(default_value = "")]
        query: String,
        #[command(flatten)]
        page: Page,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Show an item")]
    Show {
        #[arg(value_name = "ITEM", help = ITEM)]
        item: String,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Change the title of an item")]
    Title {
        #[arg(value_name = "ITEM", help = ITEM)]
        item: String,
        title: String,
    },
    #[command(about = "Rate items from 0 to 5")]
    Rate {
        #[arg(value_parser = clap::value_parser!(u32).range(0..=5))]
        rating: u32,
        #[arg(required = true, value_name = "ITEM", help = ITEM)]
        items: Vec<String>,
    },
    #[command(about = "Mark items as favorites")]
    Fav {
        #[arg(required = true, value_name = "ITEM", help = ITEM)]
        items: Vec<String>,
        #[arg(long, help = "Unmark them instead")]
        off: bool,
    },
    #[command(about = "Add or remove tags", group(ArgGroup::new("change").required(true).multiple(true)))]
    Tag {
        #[arg(required = true, value_name = "ITEM", help = ITEM)]
        items: Vec<String>,
        #[arg(long, value_name = "TAG", group = "change")]
        add: Vec<String>,
        #[arg(long, value_name = "TAG", group = "change")]
        remove: Vec<String>,
    },
    #[command(about = "List the tags with their item counts")]
    Tags {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Rename a tag, merging it into an existing one")]
    Retag { from: String, to: String },
    #[command(about = "Print the path of the thumbnail of an item, creating it when needed")]
    Thumb {
        #[arg(value_name = "ITEM", help = ITEM)]
        item: String,
        #[arg(long, default_value = "512", value_parser = ["256", "512", "1024"])]
        size: String,
    },
    #[command(about = "Show mature thumbnails of packages, or allow photosensitive packages to be shown")]
    Confirm {
        #[arg(value_parser = ["mature", "photosensitive"])]
        what: String,
        #[arg(required = true, value_name = "ITEM", help = ITEM)]
        items: Vec<String>,
    },
    #[command(about = "Show an item on the outputs")]
    Apply {
        #[arg(value_name = "ITEM", help = ITEM)]
        item: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
        #[arg(short, long, help = FIT)]
        fit: Option<Fit>,
        #[arg(short, long, default_value = "", help = "Preset uid or name")]
        preset: String,
    },
}

pub async fn run(client: &Client, command: Lib) -> Result<()> {
    let lib = &client.library;
    match command {
        Lib::Add { dir, no_recursive, wait } => {
            let dir = path_arg(&dir)?;
            let start = lib.add_source(&dir, !no_recursive).inspect_ok(|(source, _)| println!("{source}"));
            job(client, start.map_ok(|(_, id)| ((), id)), &wait).await?
        }
        Lib::Sources { json } => show::reply(lib.list_sources().await?, json, sources)?,
        Lib::Scan { source, wait } => {
            job(client, lib.rescan(source.as_deref().unwrap_or("")).map_ok(|id| ((), id)), &wait).await?
        }
        Lib::Unsource { source } => lib.remove_source(&source).await?,
        Lib::Import { files, copy, wait } => {
            let paths = files.iter().map(|file| path_arg(file)).collect::<Result<Vec<_>>>()?;
            job(client, lib.import(&strs(&paths), copy).map_ok(|id| ((), id)), &wait).await?
        }
        Lib::Rm { items, delete_files } => {
            lib.remove(&strs(&resolve(client, &items).await?), delete_files).await?
        }
        Lib::Search { query, page, json } => {
            show::reply(lib.search(&query, page.offset, page.limit).await?, json, search)?
        }
        Lib::Show { item, json } => {
            show::reply(lib.get_item(&one(client, &item).await?).await?, json, details)?
        }
        Lib::Title { item, title } => lib.set_title(&one(client, &item).await?, &title).await?,
        Lib::Rate { rating, items } => lib.set_rating(&strs(&resolve(client, &items).await?), rating).await?,
        Lib::Fav { items, off } => lib.set_favorite(&strs(&resolve(client, &items).await?), !off).await?,
        Lib::Tag { items, add, remove } => {
            lib.set_tags(&strs(&resolve(client, &items).await?), &strs(&add), &strs(&remove)).await?
        }
        Lib::Tags { json } => show::reply(lib.list_tags().await?, json, tags)?,
        Lib::Retag { from, to } => lib.rename_tag(&from, &to).await?,
        Lib::Thumb { item, size } => {
            println!("{}", lib.thumbnail(&one(client, &item).await?, size.parse()?).await?)
        }
        Lib::Confirm { what, items } => lib.confirm(&strs(&resolve(client, &items).await?), &what).await?,
        Lib::Apply { item, output, fit, preset } => {
            let uid = one(client, &item).await?;
            crate::caution::apply(client, &uid, &output, fit.map_or("", Fit::as_str), &preset).await?
        }
    }
    Ok(())
}
