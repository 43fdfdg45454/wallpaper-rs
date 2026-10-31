use super::{CALMING, ENGINE, LEFT, LEVEL, RETRIES};
use std::sync::PoisonError;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{SystemTime, UNIX_EPOCH};
use wprs_scene::Why;

pub(crate) fn retry() {
    wprs_render::retry_effects();
    if let Err(e) = wprs_scene::forgive_strikes() {
        log::warn!("cannot forget the effects that stopped the GPU or the app: {e}");
    }
    match crate::library::with(|lib| Ok(lib.recheck(wprs_scene::STOPPED)?)) {
        Ok(uids) if uids.is_empty() => {}
        Ok(_) => crate::store::changed(),
        Err(e) => log::warn!("cannot check the left-out packages again: {e}"),
    }
    RETRIES.fetch_add(1, Relaxed);
}

pub(crate) fn finish() {
    if !wprs_render::stop_builds() {
        log::warn!("a package shader is still building while the wallpaper closes");
    }
    wprs_scene::end_trials();
}

pub(crate) fn flashes() -> &'static str {
    ["", "slowed", "paused"][CALMING.load(Relaxed).min(2) as usize]
}

pub(crate) fn particles() -> &'static str {
    ["", "gpu", "cpu"][ENGINE.load(Relaxed).min(2) as usize]
}

pub(crate) fn left_out() -> Vec<serde_json::Value> {
    let left = LEFT.lock().unwrap_or_else(PoisonError::into_inner);
    left.1.iter().map(|(id, reason)| serde_json::json!({ "id": id, "reason": reason })).collect()
}

pub(crate) fn level() -> Option<[String; 3]> {
    let level = LEVEL.load(Relaxed).checked_sub(1)?;
    let why = Why::ALL.get(level as usize % 8)?.name();
    Some([format!("T{}", level / 32), format!("Q{}", level / 8 % 4), why.into()])
}

pub(crate) fn wall() -> f64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = libc::time_t::try_from(now.as_secs()).unwrap_or_default();
    let mut local = unsafe { std::mem::zeroed::<libc::tm>() };
    let found = !unsafe { libc::localtime_r(&seconds, &mut local) }.is_null();
    now.as_secs_f64() + if found { local.tm_gmtoff as f64 } else { 0.0 }
}
