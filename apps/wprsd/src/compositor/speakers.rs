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

pub(super) struct Speaker {
    pack: Arc<ScenePack>,
    output: String,
    track: Option<Soundtrack>,
    playing: bool,
    volume: f64,
    voices: Vec<Voice>,
    fade: Duration,
    quiet: Option<Instant>,
}

pub(super) fn choose(shown: Vec<(String, PathBuf, bool, f64)>, settings: Settings, locked: bool) -> Chosen {
    let mut chosen = Chosen::new();
    let volume = |level: f64| if settings.muted { 0.0 } else { settings.volume * level };
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
            if live && scene.heard.is_some_and(|at| at <= now) {
                scene.hear(interval, now);
            }
            if self.speakers.get(&path).is_some_and(|s| !Arc::ptr_eq(&s.pack, &scene.spec.pack)) {
                self.speakers.remove(&path);
            }
            let silent = scene.voices.iter().all(|v| !v.play || v.volume <= 0.0);
            let speaker = match self.speakers.entry(path) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(_) if !live || silent => continue,
                Entry::Vacant(entry) => entry.insert(Speaker::open(&scene.spec.pack, &scene.voices, volume)),
            };
            speaker.output = name;
            speaker.quiet = silent.then(|| speaker.quiet.unwrap_or(now));
            let playing = live && !speaker.faded(now);
            let track = speaker.track.as_ref();
            if speaker.voices != scene.voices {
                track.iter().for_each(|t| t.set(&scene.voices));
                speaker.voices.clone_from(&scene.voices);
            }
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
    fn open(pack: &Arc<ScenePack>, voices: &[Voice], volume: f64) -> Speaker {
        let sounds: Vec<_> = pack.scene.sounds().collect();
        let lanes: Vec<(Option<usize>, Track)> = (sounds.iter())
            .map(|s| {
                let file = pack.sounds.iter().position(|(id, _)| *id == s.sound);
                let track = Track { repeat: s.repeat, fade_in: s.fade_in, fade_out: s.fade_out };
                (file.filter(|_| s.play != Value::Fixed(false)), track)
            })
            .collect();
        let failed = |event| {
            if let Event::Error(e) = event {
                tracing::warn!("the scene sound stopped: {e}");
            }
        };
        let track = Soundtrack::open(&pack.clips, &lanes, voices, volume, failed);
        let track = track.inspect_err(|e| tracing::warn!("cannot play the scene sound: {e}")).ok();
        tracing::info!(scene = pack.id, "scene sound on");
        let fade = sounds.iter().map(|s| Duration::from_secs_f32(s.fade_out)).max().unwrap_or_default();
        let (pack, output, voices) = (pack.clone(), String::new(), voices.to_vec());
        Speaker { pack, output, track, playing: false, volume, voices, fade, quiet: None }
    }

    fn faded(&self, now: Instant) -> bool {
        self.quiet.is_some_and(|at| at + self.fade <= now)
    }

    fn due(&self) -> Option<Instant> {
        let faded = self.quiet? + self.fade;
        Some(if self.playing { faded } else { faded + LINGER })
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        tracing::info!(scene = self.pack.id, "scene sound off");
    }
}

#[cfg(test)]
mod tests {
    use super::super::Content;
    use super::super::output::{Shown, Sink};
    use super::*;
    use crate::compositor::SceneSpec;
    use wprs_core::Fit;
    use wprs_scene::{Scene, Tier};

    #[test]
    fn scenes_start_muted_play_on_one_output_and_never_while_locked() {
        let pond = PathBuf::from("/pond.wpak");
        let shown = |live: [bool; 2]| {
            vec![("A".into(), pond.clone(), live[0], 1.0), ("B".into(), pond.clone(), live[1], 0.5)]
        };
        let muted = crate::daemon::settings(&wprs_core::Config::default());
        assert!(choose(shown([true, true]), muted, false).is_empty());
        let heard = Settings { muted: false, ..muted };
        let one = Chosen::from([(pond.clone(), ("A".into(), true, 0.5))]);
        assert_eq!(choose(shown([true, true]), heard, false), one);
        assert_eq!(choose(shown([false, true]), heard, false)[&pond], ("B".into(), true, 0.25));
        assert_eq!(choose(shown([false, false]), heard, false)[&pond], ("A".into(), false, 0.5));
        assert_eq!(choose(shown([true, true]), heard, true)[&pond], ("A".into(), false, 0.5), "locked");
        assert!(choose(shown([true, true]), Settings { volume: 0.0, ..heard }, false).is_empty());
        let quiet_first = vec![("A".into(), pond.clone(), true, 0.0), ("B".into(), pond.clone(), true, 0.5)];
        assert_eq!(choose(quiet_first, heard, false)[&pond], ("B".into(), true, 0.25), "A is turned down");
    }

    fn delayed() -> SceneState {
        let text = serde_json::json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [16, 9] },
            "assets": { "wind": { "path": "wind.ogg" } }, "timelines": [{ "id": "t", "duration": 9 }],
            "layers": [{ "id": "wind", "type": "sound", "sound": "wind",
                         "play": { "timeline": "t", "keys": [[0, false], [1.5, true]] } }] });
        let scene = Arc::new(Scene::from_json(&text.to_string()).unwrap());
        let sounds = vec![("wind".into(), PathBuf::from("/wind.ogg"))];
        let (path, id, title, properties, presets, assets, videos, clips, scripts) = Default::default();
        let pack = Arc::new(ScenePack {
            path,
            id,
            title,
            languages: Vec::new(),
            locale: String::new(),
            scene,
            properties,
            presets,
            assets,
            videos,
            sounds,
            clips,
            scripts,
        });
        let spec = SceneSpec { values: Default::default(), pack, extra: Vec::new(), hold: None };
        SceneState::new(spec, super::super::power::Power { tier: Tier::T3, on_battery: false, max_fps: 30 })
    }

    #[test]
    fn the_scene_clock_of_a_sound_stops_while_the_scene_pauses() {
        let mut scene = delayed();
        let (start, tick) = (Instant::now(), Duration::from_millis(100));
        let mut at = |seconds: f64| {
            scene.hear(tick, start + Duration::from_secs_f64(seconds));
            (scene.voices[0].play, scene.heard.map(|at| at - start))
        };
        assert_eq!([at(0.0), at(1.0)], [(false, Some(tick)), (false, Some(Duration::from_millis(1100)))]);
        scene.pause();
        let mut at = |seconds: f64| {
            scene.hear(tick, start + Duration::from_secs_f64(seconds));
            scene.voices[0].play
        };
        assert_eq!([at(5.0), at(5.4), at(5.6)], [false, false, true]);
    }

    #[test]
    fn a_silent_scene_wakes_the_loop_on_its_clock_before_any_speaker_exists() {
        let (mut scene, now, tick) = (delayed(), Instant::now(), Duration::from_millis(100));
        scene.hear(tick, now);
        let (content, background) = (Content::Scene(scene.spec.clone()), [0; 3]);
        let (image, transition, uv, relief, scene) = (None, None, None, None, Some(scene));
        let shown = Shown { content, fit: Fit::Stretch, background, image, transition, uv, relief, scene };
        let outputs = BTreeMap::from([("A".into(), Output::new((16, 9), shown, Sink::Stream(None), 60))]);
        let chosen = |live| Chosen::from([(PathBuf::new(), ("A".into(), live, 1.0))]);
        assert_eq!(ticks(&outputs, chosen(true)).collect::<Vec<_>>(), [now + tick]);
        assert_eq!(ticks(&outputs, chosen(false)).count(), 0, "a paused scene sleeps");
    }
}
