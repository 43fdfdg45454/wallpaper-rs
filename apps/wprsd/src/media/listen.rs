use super::track::number;
use super::{Frame, Player, Status, Track, chosen};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use wprs_core::model::MediaStatus;
use wprs_scene::MediaState;
use zbus::fdo::DBusProxy;
use zbus::message::Type;
use zbus::{Connection, MatchRule, MessageStream, OwnedVariant};

const SPACE: &str = "org.mpris.MediaPlayer2";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const PATIENCE: Duration = Duration::from_secs(2);

type Props = HashMap<String, OwnedVariant>;

#[derive(Default)]
pub(super) struct Reading {
    pub(super) players: HashMap<String, Player>,
    names: HashMap<String, String>,
    track: u64,
    last: Option<Track>,
}

pub(super) async fn listen(bus: Connection) {
    if let Err(e) = run(&bus).await {
        tracing::warn!("cannot follow the media players: {e}");
    }
}

async fn run(bus: &Connection) -> zbus::Result<()> {
    let owners = MatchRule::builder().msg_type(Type::Signal).sender("org.freedesktop.DBus");
    let owners = owners.interface("org.freedesktop.DBus").member("NameOwnerChanged").arg0ns(SPACE).build()?;
    let changes =
        MatchRule::builder().msg_type(Type::Signal).interface(PROPERTIES).member("PropertiesChanged");
    let changes = changes.path(PATH).arg(0, PLAYER).build()?;
    let seeks =
        MatchRule::builder().msg_type(Type::Signal).interface(PLAYER).member("Seeked").path(PATH).build()?;
    let mut owners = MessageStream::for_match_rule(owners, bus, None).await?;
    let mut changes = MessageStream::for_match_rule(changes, bus, None).await?;
    let mut seeks = MessageStream::for_match_rule(seeks, bus, None).await?;
    let dbus = DBusProxy::new(bus).await?;
    let mut r = Reading::default();
    for name in dbus.list_names().await? {
        let name = name.to_string();
        if name.strip_prefix(SPACE).is_some_and(|rest| rest.starts_with('.'))
            && let Ok(owner) = dbus.get_name_owner(name.as_str().try_into()?).await
        {
            r.add(bus, name, owner.to_string()).await;
        }
    }
    r.publish();
    loop {
        tokio::select! {
            Some(Ok(m)) = owners.next() => {
                let Ok((name, old, new)) = m.body().deserialize::<(String, String, String)>() else { continue };
                if !old.is_empty() {
                    r.players.remove(&old);
                    r.names.remove(&name);
                }
                if !new.is_empty() && name.starts_with(&format!("{SPACE}.")) {
                    r.add(bus, name, new).await;
                }
            }
            Some(Ok(m)) = changes.next() => {
                let Some(sender) = m.header().sender().map(|s| s.to_string()) else { continue };
                let Ok((_, props, gone)) = m.body().deserialize::<(String, Props, Vec<String>)>() else { continue };
                r.change(bus, &sender, &props, !gone.is_empty()).await;
            }
            Some(Ok(m)) = seeks.next() => {
                let Some(sender) = m.header().sender().map(|s| s.to_string()) else { continue };
                let Ok((at,)) = m.body().deserialize::<(i64,)>() else { continue };
                if let Some(p) = r.players.get_mut(&sender) {
                    p.seek(at as f64, Instant::now());
                }
            }
            else => return Ok(()),
        }
        r.publish();
    }
}

async fn call(bus: &Connection, owner: &str, property: Option<&str>) -> Option<zbus::Message> {
    let call = async {
        match property {
            Some(p) => bus.call_method(Some(owner), PATH, Some(PROPERTIES), "Get", &(PLAYER, p)).await,
            None => bus.call_method(Some(owner), PATH, Some(PROPERTIES), "GetAll", &(PLAYER,)).await,
        }
    };
    tokio::time::timeout(PATIENCE, call).await.ok()?.ok()
}

async fn all(bus: &Connection, owner: &str) -> Option<Props> {
    call(bus, owner, None).await.and_then(|m| m.body().deserialize::<Props>().ok())
}

impl Reading {
    async fn add(&mut self, bus: &Connection, name: String, owner: String) {
        let props = all(bus, &owner).await.unwrap_or_default();
        let mut player = Player::new(Instant::now());
        player.apply(&props, Instant::now());
        self.names.insert(name, owner.clone());
        self.players.insert(owner, player);
    }

    async fn change(&mut self, bus: &Connection, sender: &str, props: &Props, stale: bool) {
        let Some(p) = self.players.get_mut(sender) else { return };
        let props = match stale {
            true => all(bus, sender).await.unwrap_or_else(|| props.clone()),
            false => props.clone(),
        };
        if p.apply(&props, Instant::now()) && !props.contains_key("Position") {
            let found = call(bus, sender, Some("Position")).await;
            let found = found.and_then(|m| m.body().deserialize::<OwnedVariant>().ok());
            if let Some(at) = found.as_ref().and_then(|v| number(v)) {
                p.seek(at, Instant::now());
            }
        }
    }

    pub(super) fn publish(&mut self) {
        let now = Instant::now();
        let active = chosen(&self.players).map(|(_, p)| p.clone()).filter(|p| p.track.known());
        let mut state = MediaState { enabled: true, rate: 1.0, ..MediaState::default() };
        if let Some(p) = &active {
            if self.last.as_ref().is_none_or(|t| !t.same_song(&p.track)) {
                (self.track, self.last) = (self.track + 1, Some(p.track.clone()));
            }
            p.track.fill(&mut state);
            state.track = self.track;
            (state.playing, state.position, state.rate) =
                (p.status == Status::Playing, p.now(now) as f32, p.rate as f32);
        }
        let playing = state.playing;
        let status = MediaStatus {
            listening: true,
            players: self.players.len() as u32,
            playing,
            track: active.is_some(),
            ..MediaStatus::default()
        };
        super::publish(Some(Frame { read: now, ..Frame::new(state) }), status);
    }
}
