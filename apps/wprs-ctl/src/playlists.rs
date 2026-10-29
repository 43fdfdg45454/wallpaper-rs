use crate::cli::{ITEM, JSON, Out, Page};
use crate::input::{resolve, strs};
use crate::items::{items, range};
use crate::show::{self, list, short, table, text, time};
use crate::usage;
use anyhow::Result;
use clap::Subcommand;
use std::collections::{BTreeMap, BTreeSet};
use wprs_client::{Client, parse};
use wprs_core::model::{HistoryReply, PlaylistPage, PlaylistsReply, Rules};

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

pub async fn run(client: &Client, command: Playlist) -> Result<()> {
    let pl = &client.playlists;
    match command {
        Playlist::Ls { json } => show::reply(pl.list().await?, json, playlists)?,
        Playlist::Show { playlist, page, json } => {
            show::reply(pl.get(&playlist, page.offset, page.limit).await?, json, self::page)?
        }
        Playlist::New { name, query } => println!("{}", pl.create(&name, &query).await?),
        Playlist::Rename { playlist, name } => pl.rename(&playlist, &name).await?,
        Playlist::Query { playlist, query } => pl.set_query(&playlist, &query).await?,
        Playlist::Rm { playlist } => pl.delete(&playlist).await?,
        Playlist::Add { playlist, items } => {
            pl.add_items(&playlist, &strs(&resolve(client, &items).await?)).await?
        }
        Playlist::Remove { playlist, items } => {
            pl.remove_items(&playlist, &strs(&resolve(client, &items).await?)).await?
        }
        Playlist::RemoveAt { playlist, indexes } => {
            let mut uids = BTreeMap::new();
            for &index in indexes.iter().collect::<BTreeSet<_>>() {
                if !uids.contains_key(&index) {
                    let page: PlaylistPage = parse(&pl.get(&playlist, index, 500).await?)?;
                    uids.extend((index..).zip(page.items.into_iter().map(|item| item.uid)));
                }
            }
            let at = |i: &u32| uids.get(i).map(String::as_str);
            let expected =
                indexes.iter().map(|i| at(i).ok_or_else(|| usage(format!("the playlist has no entry {i}"))));
            pl.remove_entries(&playlist, &indexes, &expected.collect::<Result<Vec<_>>>()?).await?
        }
        Playlist::Move { playlist, from, to } => {
            let page: PlaylistPage = parse(&pl.get(&playlist, from, 1).await?)?;
            let expected =
                page.items.first().ok_or_else(|| usage(format!("the playlist has no entry {from}")))?;
            pl.move_item(&playlist, from, to, &expected.uid).await?
        }
        Playlist::Rule { playlist, key: Some(key), value: Some(value), .. } => {
            pl.set_rule(&playlist, &key, &value).await?
        }
        Playlist::Rule { playlist, key, json, .. } => {
            let reply = pl.get(&playlist, 0, 0).await?;
            let text = match json {
                true => show::raw(&reply)?,
                false => show::pick(rules(&parse::<PlaylistPage>(&reply)?.playlist.rules), key.as_deref())?,
            };
            print!("{text}")
        }
        Playlist::Start { playlist, out } => {
            crate::inputs::asking(client, &out.output, pl.start(&playlist, &out.output)).await?
        }
        Playlist::Stop(out) => pl.stop(&out.output).await?,
        Playlist::History { output, limit, json } => {
            show::reply(pl.history(&output, limit).await?, json, history)?
        }
    }
    Ok(())
}

pub fn rules(rules: &Rules) -> Vec<(&'static str, String)> {
    vec![
        ("order", text(&rules.order)),
        ("interval", rules.interval.clone()),
        ("at", rules.at.join(",")),
        ("days", rules.days.join(",")),
        ("synced", rules.synced.to_string()),
        ("resume", rules.resume.to_string()),
        ("advance_while_paused", rules.advance_while_paused.to_string()),
        ("skip_videos_on_battery", rules.skip_videos_on_battery.to_string()),
        ("transition_ms", rules.transition_ms.map_or(String::new(), |ms| ms.to_string())),
    ]
}

pub fn playlists(reply: PlaylistsReply) -> String {
    let rows = reply.playlists.into_iter().map(|p| {
        let kind = if p.query.is_some() { "smart" } else { "manual" }.into();
        let (count, order) = (p.count.to_string(), text(&p.rules.order));
        vec![short(&p.uid), p.name, kind, count, order, p.rules.interval, list(&p.outputs)]
    });
    table(&["PLAYLIST", "NAME", "TYPE", "COUNT", "ORDER", "INTERVAL", "OUTPUTS"], rows)
}

pub fn page(page: PlaylistPage) -> String {
    let (p, count) = (page.playlist, range(page.offset, page.items.len(), page.total));
    let fields = [("playlist", p.uid), ("name", p.name)].into_iter().chain(p.query.map(|q| ("query", q)));
    show::lines(fields.chain([("outputs", list(&p.outputs)), ("items", count)]))
        + &items(&page.items, Some(page.offset))
}

pub fn history(reply: HistoryReply) -> String {
    let rows = reply.entries.into_iter().map(|e| match e.item {
        Some(item) => vec![time(e.at), short(&item.uid), item.title],
        None => vec![time(e.at), "-".into(), "(removed)".into()],
    });
    table(&["TIME", "ITEM", "TITLE"], rows)
}
