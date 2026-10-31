use super::{Playlist, history, playlists, rules};
use crate::input::{resolve, strs};
use crate::show;
use crate::usage;
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
use wprs_client::{Client, parse};
use wprs_core::model::PlaylistPage;

pub async fn run(client: &Client, command: Playlist) -> Result<()> {
    let pl = &client.playlists;
    match command {
        Playlist::Ls { json } => show::reply(pl.list().await?, json, playlists)?,
        Playlist::Show { playlist, page, json } => {
            show::reply(pl.get(&playlist, page.offset, page.limit).await?, json, super::page)?
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
