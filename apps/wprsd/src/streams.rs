use crate::daemon::{Daemon, State, output_name};
use crate::wallpaper::Upload;
use std::collections::HashMap;
use wprs_core::dbus;
use wprs_pipewire::Reply;
use zbus::fdo;

pub(crate) const HELD: &str = "";

#[derive(Default)]
pub(crate) struct Claim {
    count: u32,
    covered: bool,
}

#[derive(Default)]
pub(crate) struct OutputInfo {
    pub(crate) size: (u32, u32),
    pub(crate) claims: HashMap<String, Claim>,
    pub(crate) uploaded: Option<Upload>,
    pub(crate) screen: Option<wprs_core::Screen>,
    pub(crate) scene: Option<std::sync::Arc<crate::scenes::ScenePack>>,
}

impl OutputInfo {
    pub(crate) fn covered(&self) -> bool {
        !self.claims.is_empty() && self.claims.values().all(|c| c.covered)
    }

    fn claimant<'a>(&self, client: &'a str) -> &'a str {
        if self.claims.contains_key(client) { client } else { HELD }
    }
}

impl Daemon {
    pub fn check_request(&self, output: &str, width: u32, height: u32) -> fdo::Result<String> {
        let max = self.compositor.max_edge().min(dbus::MAX_STREAM_EDGE);
        let fits = width.max(height) <= max && self.compositor.check_target(width, height).is_ok();
        let error = || {
            let limits = format!("edges 1..={max} and one frame within the GPU readback limit");
            fdo::Error::InvalidArgs(format!("invalid size {width}x{height} ({limits})"))
        };
        fits.then_some(()).ok_or_else(error)?;
        output_name(output)
    }

    pub fn claim(&self, output: &str, client: &str, size: (u32, u32), reply: Reply) -> fdo::Result<String> {
        let output = self.check_request(output, size.0, size.1)?;
        let mut st = self.state();
        let info = st.outputs.entry(output.clone()).or_default();
        info.size = size;
        let claim = info.claims.entry(client.to_owned()).or_default();
        claim.count = if client == HELD { 1 } else { claim.count + 1 };
        self.exporter.ensure_stream(&output, size.0, size.1, reply);
        self.activity(&st);
        self.changes.changed("state", vec![output.clone()]);
        Ok(output)
    }

    pub(crate) fn release(&self, output: &str, client: &str) -> anyhow::Result<()> {
        let output = output_name(output)?;
        let mut st = self.state();
        let client = st.outputs.get(&output).map_or(HELD, |info| info.claimant(client));
        self.unclaim(&mut st, &output, client, 1)?;
        self.respan(st);
        Ok(())
    }

    pub fn drop_client(&self, client: &str) -> anyhow::Result<()> {
        let mut st = self.state();
        let held =
            |(name, info): (&String, &OutputInfo)| info.claims.contains_key(client).then(|| name.clone());
        let outputs: Vec<String> = st.outputs.iter().filter_map(held).collect();
        if !outputs.is_empty() {
            tracing::debug!(client, "a client left the bus");
        }
        outputs.iter().try_for_each(|output| self.unclaim(&mut st, output, client, u32::MAX))
    }

    pub fn unclaim(&self, st: &mut State, output: &str, client: &str, count: u32) -> anyhow::Result<()> {
        let Some(info) = st.outputs.get_mut(output) else { return Ok(()) };
        if let Some(claim) = info.claims.get_mut(client) {
            claim.count = claim.count.saturating_sub(count);
            if claim.count == 0 {
                info.claims.remove(client);
            }
        }
        if info.claims.is_empty() {
            st.outputs.remove(output);
            self.compositor.remove(output);
            self.exporter.remove_stream(output)?;
            tracing::info!(output, "stream released");
        }
        self.activity(st);
        self.changes.changed("state", vec![output.to_owned()]);
        Ok(())
    }

    pub(crate) fn set_covered(&self, output: &str, client: &str, covered: bool) -> anyhow::Result<()> {
        let output = output_name(output)?;
        let mut st = self.state();
        let changed = (st.outputs.get_mut(&output))
            .and_then(|info| info.claims.get_mut(info.claimant(client)))
            .is_some_and(|c| std::mem::replace(&mut c.covered, covered) != covered);
        if changed {
            tracing::debug!(output, covered, "output coverage changed");
            self.activity(&st);
            self.changes.changed("state", vec![output]);
        }
        Ok(())
    }
}
