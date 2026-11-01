use super::Settings;
use super::output::Output;
use super::scene::SceneState;
use super::state::State;
use crate::scenes::ScenePack;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_scene::{VOLUME, Value, Voice};
use wprs_video::{Event, Soundtrack, Track};

type Chosen = BTreeMap<PathBuf, (String, bool, f64)>;

const LINGER: Duration = Duration::from_secs(5);
const RESYNC: Duration = Duration::from_secs(1);

pub(super) struct Speaker {
    pack: Arc<ScenePack>,
    output: String,
    track: Option<Soundtrack>,
    playing: bool,
    volume: f64,
    voices: Vec<Voice>,
    fade: Duration,
    quiet: Option<Instant>,
    synced: Instant,
}

pub(super) fn choose(shown: Vec<(String, PathBuf, bool, f64)>, settings: Settings, locked: bool) -> Chosen {
    let mut chosen = Chosen::new();
    let volume = |level: f64| {
        if settings.muted || !settings.scene_sound { 0.0 } else { settings.volume * settings.duck * level }
    };
    let audible =
        shown.into_iter().map(|(name, pack, active, level)| (name, pack, active && !locked, volume(level)));
    for (name, pack, live, volume) in audible.filter(|(.., volume)| *volume > 0.0) {
        let entry = chosen.entry(pack).or_insert_with(|| (name.clone(), live, volume));
        if live && !entry.1 {
            *entry = (name, true, volume);
        }
    }
    chosen
}

fn sounding(o: &Output) -> Option<&SceneState> {
    o.shown.scene.as_ref().filter(|s| !s.spec.pack.sounds.is_empty())
}

fn ticks(outputs: &BTreeMap<String, Output>, chosen: Chosen) -> impl Iterator<Item = Instant> + '_ {
    let live = chosen.into_values().filter(|(_, live, _)| *live);
    live.filter_map(|(name, ..)| sounding(outputs.get(&name)?)?.heard)
}

fn level(o: &Output) -> f64 {
    let percent = o.shown.scene.as_ref().and_then(|s| s.spec.values.get(VOLUME)?.as_f64());
    percent.map_or(1.0, |v| (v / 100.0).clamp(0.0, 1.0))
}

impl State {
    fn chosen(&self) -> Chosen {
        let live =
            |name: &String, o: &Output| o.active && !o.covered && o.watched(self.exporter.is_streaming(name));
        let shown = (self.outputs.iter()).filter_map(|(name, o)| {
            Some((name.clone(), sounding(o)?.spec.pack.path.clone(), live(name, o), level(o)))
        });
        choose(shown.collect(), self.settings, self.locked)
    }

    pub(super) fn sync_sounds(&mut self) {
        let (now, exporter) = (Instant::now(), &self.exporter);
        let unwatched = self.outputs.iter_mut().filter(|(name, o)| !o.watched(exporter.is_streaming(name)));
        for scene in unwatched.filter_map(|(_, o)| o.shown.scene.as_mut()) {
            if !scene.spec.pack.sounds.is_empty() {
                scene.hold();
            }
        }
        let (chosen, power) = (self.chosen(), self.power);
        self.speakers.retain(|path, s| {
            chosen.contains_key(path) && s.quiet.is_none_or(|at| at + s.fade + LINGER > now)
        });
        for (path, (name, live, volume)) in chosen {
            let Some(o) = self.outputs.get_mut(&name) else { continue };
            let interval = Duration::from_secs(1) / o.fps(power);
            let Some(scene) = o.shown.scene.as_mut() else { continue };
            let resync = self.speakers.get(&path).is_some_and(|s| s.playing && s.synced + RESYNC <= now);
            let heard = live && (resync || scene.heard.is_some_and(|at| at <= now));
            if heard {
                scene.hear(interval, now);
            }
            if self.speakers.get(&path).is_some_and(|s| !Arc::ptr_eq(&s.pack, &scene.spec.pack)) {
                self.speakers.remove(&path);
            }
            let ended =
                self.speakers.get(&path).and_then(|s| s.track.as_ref()).is_some_and(Soundtrack::quiet);
            let silent = ended || scene.voices.iter().all(|v| !v.play || v.volume <= 0.0);
            let speaker = match self.speakers.entry(path) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(_) if !live || silent => continue,
                Entry::Vacant(entry) => {
                    entry.insert(Speaker::open(&scene.spec.pack, &scene.voices, &scene.tracks(), volume))
                }
            };
            speaker.output = name;
            speaker.quiet = silent.then(|| speaker.quiet.unwrap_or(now));
            let playing = live && !speaker.faded(now);
            if heard || speaker.voices != scene.voices {
                speaker.follow(&scene.voices, now);
            }
            let track = speaker.track.as_ref();
            if speaker.volume != volume {
                track.iter().for_each(|t| t.set_volume(volume));
                speaker.volume = volume;
            }
            if speaker.playing != playing {
                track.iter().for_each(|t| t.set_playing(playing));
                speaker.playing = playing;
            }
        }
    }

    pub(super) fn heard(&self) -> impl Iterator<Item = Instant> + '_ {
        ticks(&self.outputs, self.chosen()).chain(self.speakers.values().filter_map(Speaker::due))
    }

    pub(super) fn sound(&self, name: &str, o: &Output) -> &'static str {
        let Some(scene) = sounding(o) else { return "" };
        match self.speakers.get(&scene.spec.pack.path) {
            Some(s) if s.output != name => "elsewhere",
            Some(s) if s.playing => "playing",
            Some(s) if s.faded(Instant::now()) => "off",
            Some(_) => "paused",
            None => "off",
        }
    }
}

impl Speaker {
    fn open(pack: &Arc<ScenePack>, voices: &[Voice], tracks: &[Track], volume: f64) -> Speaker {
        let sounds: Vec<_> = pack.scene.sounds().collect();
        let file = |id: &str| pack.sounds.iter().position(|(sound, _)| sound == id);
        let lanes: Vec<(Vec<Option<usize>>, Track)> = (sounds.iter().zip(tracks))
            .map(|(s, track)| {
                let silent = s.play == Value::Fixed(false);
                (s.clips().map(|id| file(id).filter(|_| !silent)).collect(), track.clone())
            })
            .collect();
        let failed = |event| {
            if let Event::Error(e) = event {
                tracing::warn!("the scene sound stopped: {e}");
            }
        };
        let track = Soundtrack::open(&pack.clips, &lanes, voices, volume, failed);
        let track = track.inspect_err(|e| tracing::warn!("cannot play the scene sound: {e}")).ok();
        tracing::info!(scene = pack.props.id, "scene sound on");
        let fade = sounds.iter().map(|s| Duration::from_secs_f32(s.fade_out)).max().unwrap_or_default();
        let (pack, output, voices) = (pack.clone(), String::new(), voices.to_vec());
        let synced = Instant::now();
        Speaker { pack, output, track, playing: false, volume, voices, fade, quiet: None, synced }
    }

    fn follow(&mut self, voices: &[Voice], now: Instant) {
        if self.voices != voices {
            self.track.iter().for_each(|t| t.set(voices));
            self.voices = voices.to_vec();
        }
        self.synced = now;
    }

    fn faded(&self, now: Instant) -> bool {
        self.quiet.is_some_and(|at| at + self.fade <= now)
    }

    fn due(&self) -> Option<Instant> {
        let faded =
            self.quiet.map(|quiet| quiet + self.fade + if self.playing { Duration::ZERO } else { LINGER });
        faded.into_iter().chain(self.playing.then_some(self.synced + RESYNC)).min()
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        tracing::info!(scene = self.pack.props.id, "scene sound off");
    }
}

#[cfg(test)]
mod tests;
