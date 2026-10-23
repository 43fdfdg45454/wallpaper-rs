use crate::show::{self, bytes, list, prefix, short, table, text, time, when};
use wprs_core::model::{ItemDetails, ItemSummary, SearchPage, SourcesReply, TagsReply};

pub fn sources(reply: SourcesReply) -> String {
    let head = ["SOURCE", "ITEMS", "SUBFOLDERS", "SCANNED", "PROVIDER", "PATH"];
    table(
        &head,
        reply.sources.into_iter().map(|s| {
            let path = s.path.display().to_string() + &s.error.map_or(String::new(), |e| format!(" ({e})"));
            let (items, subfolders) = (s.items.to_string(), if s.recursive { "yes" } else { "no" }.into());
            vec![short(&s.uid), items, subfolders, when(s.scanned_at), s.provider.unwrap_or("-".into()), path]
        }),
    )
}

pub fn search(page: SearchPage) -> String {
    let count = range(page.offset, page.items.len(), page.total);
    show::lines([("query", page.query), ("items", count)]) + &items(&page.items, None)
}

pub fn range(offset: u32, count: usize, total: u32) -> String {
    match count {
        0 => format!("none of {total}"),
        1 => format!("{} of {total}", offset + 1),
        _ => format!("{}-{} of {total}", offset + 1, offset as usize + count),
    }
}

pub fn items(items: &[ItemSummary], first: Option<u32>) -> String {
    let head = ["#", "ITEM", "KIND", "SIZE", "RATING", "FLAGS", "TITLE", "TAGS"];
    table(
        &head[usize::from(first.is_none())..],
        items.iter().zip(0..).map(|(i, n)| {
            let flags = [(i.favorite, "favorite"), (i.missing, "missing"), (i.error.is_some(), "broken")];
            let flags = flags.into_iter().chain([(i.revoked.is_some(), "revoked")]).filter(|(on, _)| *on);
            let flags = flags.map(|(_, flag)| flag).collect::<Vec<_>>().join(",");
            let (kind, size, rating) =
                (text(&i.kind), dimensions(i.width, i.height, i.duration_ms), i.rating.to_string());
            let row = [short(&i.uid), kind, size, rating, flags, i.title.clone(), list(&i.tags)];
            first.map(|first| (first + n).to_string()).into_iter().chain(row).collect()
        }),
    )
}

pub fn dimensions(width: Option<u32>, height: Option<u32>, duration_ms: Option<u32>) -> String {
    let size = width.zip(height).map(|(w, h)| format!("{w}x{h}"));
    let duration = duration_ms.map(|ms| format!("{}:{:02}", ms / 60_000, ms / 1000 % 60));
    [size, duration].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

pub fn details(reply: ItemDetails) -> String {
    let (package, i) = (reply.item.package.clone(), reply.item);
    let presets = reply.presets.into_iter().map(|p| {
        let value = [p.value.fit.map(|f| f.to_string()), p.value.background].into_iter().flatten();
        format!("{} ({})", p.name, value.collect::<Vec<_>>().join(" "))
    });
    let credit =
        reply.credit.map(|c| list([Some(c.title), c.attribution, c.license, c.link].into_iter().flatten()));
    let fields = [
        ("item", Some(i.uid)),
        ("title", Some(i.title)),
        ("kind", Some(text(&i.kind))),
        ("path", Some(i.path.display().to_string())),
        ("size", Some(bytes(i.size))),
        ("dimensions", Some(dimensions(i.width, i.height, i.duration_ms)).filter(|d| !d.is_empty())),
        ("audio", i.audio.map(|a| a.to_string())),
        ("color", i.color.map(|c| format!("#{c:06x}"))),
        ("tags", Some(list(&i.tags))),
        ("rating", Some(i.rating.to_string())),
        ("favorite", Some(i.favorite.to_string())),
        ("added", Some(time(i.added_at))),
        ("used", i.used_at.map(|at| format!("{} ({} times)", time(at), i.use_count))),
        ("missing", i.missing.then(|| "true".into())),
        ("error", i.error),
        ("package", package.as_ref().map(|p| format!("{} {} ({}, {})", p.id, p.version, p.kind, p.license))),
        ("installed", reply.installed.map(|p| format!("{} from {}", p.version, prefix(&p.repo_id)))),
        ("revoked", reply.revoked.map(|r| text(&r))),
        ("credit", credit),
        ("presets", Some(list(presets)).filter(|p| !p.is_empty())),
        ("playlists", Some(list(reply.playlists.iter().map(|p| &p.name))).filter(|p| !p.is_empty())),
        ("thumb", reply.thumb.map(|t| t.display().to_string())),
    ];
    let about = reply.about.as_ref().map(|a| crate::about::fields(a, package.as_ref())).unwrap_or_default();
    let fields = fields.into_iter().chain(about).filter(|(_, v)| v.as_ref().is_some_and(|v| !v.is_empty()));
    show::lines(fields.filter_map(|(key, value)| Some((key, value?))))
        + &reply.about.as_ref().map(crate::about::texts).unwrap_or_default()
}

pub fn tags(reply: TagsReply) -> String {
    table(&["COUNT", "TAG"], reply.tags.into_iter().map(|t| vec![t.count.to_string(), t.name]))
}
