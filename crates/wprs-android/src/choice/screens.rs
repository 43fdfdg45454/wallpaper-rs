use super::{Choice, LOCK, PENDING, turn};
use crate::library::with;
use crate::{Error, settings, store};
use serde::Deserialize;
use std::sync::PoisonError;
use wprs_library::Turn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Target {
    Both,
    Home,
    Lock,
}

fn own(config: &wprs_core::Config) -> Option<wprs_core::config::OutputConfig> {
    config.outputs.get(LOCK).filter(|o| o.image.is_some() || o.playlist.is_some()).cloned()
}

pub(crate) fn commit(on_battery: bool, target: Target) -> Result<bool, Error> {
    let Some(choice) = PENDING.lock().unwrap_or_else(PoisonError::into_inner).take() else {
        return Ok(false);
    };
    let screens: &[bool] = match target {
        Target::Both => &[false, true],
        Target::Home => &[false],
        Target::Lock => &[true],
    };
    let path = match &choice {
        Choice::Item(uid) => Some(with(|lib| Ok(lib.item(uid)?.path))?),
        Choice::Playlist(_) => None,
    };
    let (image, playlist) = match &choice {
        Choice::Item(_) => (path, None),
        Choice::Playlist(uid) => (None, Some(uid.clone())),
    };
    settings::update(|config| {
        if target == Target::Home && own(config).is_none() {
            let (image, playlist) = (config.default.image.clone(), config.default.playlist.clone());
            let lock = config.outputs.entry(LOCK.into()).or_default();
            (lock.image, lock.playlist) = (image, playlist);
        }
        if target == Target::Both
            && let Some(lock) = config.outputs.get_mut(LOCK)
        {
            (lock.image, lock.playlist, lock.preset) = (None, None, None);
        }
        let section = match target {
            Target::Lock => config.outputs.entry(LOCK.into()).or_default(),
            _ => &mut config.default,
        };
        (section.image, section.playlist, section.preset) = (image, playlist, None);
        config.outputs.retain(|_, o| *o != Default::default());
        Ok(())
    })?;
    if let Choice::Playlist(uid) = &choice {
        let start = Turn { start: true, live: true, ..Turn::default() };
        with(|lib| screens.iter().try_for_each(|&lock| turn(lib, (uid, lock), start, on_battery).map(drop)))?;
    }
    store::changed();
    Ok(settings::chosen(settings::COLORS) != "never")
}
