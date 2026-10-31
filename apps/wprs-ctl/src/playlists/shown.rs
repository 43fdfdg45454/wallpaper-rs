use crate::items::{items, range};
use crate::show::{self, list, short, table, text, time};
use wprs_core::model::{HistoryReply, PlaylistPage, PlaylistsReply, Rules};

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
