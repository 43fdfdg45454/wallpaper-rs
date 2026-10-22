use crate::show::{self, bytes, table, text, when};
use wprs_core::model::AccountReply;

pub fn status(reply: AccountReply) -> String {
    let Some(a) = reply.account else { return "no server is added\n".into() };
    let encryption = match a.encrypted {
        Some(false) => "off: the server can read every record".into(),
        Some(true) => "end to end".into(),
        None => "-".into(),
    };
    let device = a.device.map_or("-".into(), |name| format!("{name} ({})", a.label));
    let storage = format!("{} of {}", bytes(a.used_bytes), bytes(a.quota_bytes));
    let pairs = [
        ("server", a.url),
        ("name", a.name),
        ("account", a.handle.unwrap_or("-".into())),
        ("device", device),
        ("state", text(&a.state)),
        ("encryption", encryption),
        ("synced", when(a.last_sync)),
        ("records", a.records.to_string()),
        ("storage", storage),
        ("warning", a.warning.unwrap_or("-".into())),
        ("error", a.last_error.unwrap_or("-".into())),
    ];
    let rows = a.devices.into_iter().map(|d| {
        let here = if d.current { "this computer" } else { "" };
        vec![d.name, d.label.unwrap_or("-".into()), d.platform, when(d.seen_at), here.into()]
    });
    show::lines(pairs) + &table(&["DEVICE", "NAME", "PLATFORM", "SEEN", ""], rows)
}
