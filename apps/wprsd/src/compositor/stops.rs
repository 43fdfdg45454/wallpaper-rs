use crate::daemon::Daemon;
use std::sync::Arc;

pub(crate) fn restore() {
    wprs_core::stopped_effects().inspect(|p| wprs_scene::keep_strikes(p));
}

pub(crate) fn forgive() {
    match wprs_scene::forgive_strikes() {
        Ok(struck) => tracing::info!("trying {struck} blocked effects again"),
        Err(e) => tracing::warn!("cannot forget the effects that stopped the GPU or wprsd: {e}"),
    }
}

pub(super) fn retry() {
    tracing::info!("drawing {} left-out effects again", wprs_render::retry_effects());
}

pub(crate) fn recheck(daemon: &Arc<Daemon>) {
    let refused = |lib: &mut wprs_library::Library| -> wprs_library::Result<Vec<String>> {
        let mut uids = lib.recheck(wprs_scene::STOPPED)?;
        uids.extend(lib.recheck(wprs_scene::UNCHECKED)?);
        Ok(uids)
    };
    match daemon.store.write(refused) {
        Ok(uids) if uids.is_empty() => {}
        Ok(uids) => {
            tracing::info!("checking {} left-out packages again", uids.len());
            daemon.library_changed(uids);
            daemon.analyze_job();
        }
        Err(e) => tracing::warn!("cannot check the left-out packages again: {e}"),
    }
}

pub(super) fn lost(reason: &str) -> ! {
    let drawn = wprs_render::drawn_effects();
    if !drawn.is_empty()
        && let Err(e) = wprs_scene::strike(&drawn)
    {
        tracing::warn!("cannot remember the effects drawn when the GPU stopped: {e}");
    }
    crate::fatal(reason)
}
