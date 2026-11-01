use crate::input::confirm;
use anyhow::Result;
use wprs_client::{Client, error_name};
use wprs_core::dbus::error::UNCONFIRMED;

pub const FLASHING: &str = "Warning: this wallpaper flashes or shows fast patterns that can trigger seizures \
    in people with photosensitive epilepsy.";

pub async fn apply(client: &Client, uid: &str, output: &str, fit: &str, preset: &str) -> Result<()> {
    let lib = &client.library;
    notice(client, uid).await?;
    let applied = crate::inputs::asking(client, output, lib.apply(uid, output, fit, preset)).await;
    match applied {
        Err(e) if error_name(&e) == Some(UNCONFIRMED) => {
            eprintln!("{FLASHING}");
            anyhow::ensure!(
                confirm("Show it anyway? You will not be asked again for this package.").await?,
                "not applied; run wprs-ctl lib confirm photosensitive {uid} to allow it"
            );
            lib.confirm(&[uid], "photosensitive").await?;
            crate::inputs::asking(client, output, lib.apply(uid, output, fit, preset)).await
        }
        done => done,
    }
}

pub async fn installed(client: &Client, job: Option<&wprs_core::model::Job>) -> Result<()> {
    let item = job.and_then(|j| j.output.as_ref()).and_then(|o| o.get("item")).and_then(|i| i.as_str());
    match item {
        Some(uid) => notice(client, uid).await,
        None => Ok(()),
    }
}

pub const COMPUTE: &str = "Warning: this wallpaper needs compute shaders, which the graphics of this computer lack \
    or run in software, so it may draw slowly or leave parts out.";

async fn notice(client: &Client, uid: &str) -> Result<()> {
    let details: wprs_core::model::ItemDetails = wprs_client::parse(&client.library.get_item(uid).await?)?;
    let compute = details.item.package.as_ref().is_some_and(|p| p.facets.iter().any(|f| f == "compute"));
    let state: wprs_core::model::State = wprs_client::parse(&client.daemon.get_state().await?)?;
    if compute && !state.compute {
        eprintln!("{COMPUTE}");
    }
    let exposes = details.item.package.map(|p| p.exposes).unwrap_or_default();
    if let Some(notice) = wprs_format::web_notice(&exposes) {
        eprintln!("{}\n{}", notice.local(), wprs_format::web_restrict(uid).local());
    }
    Ok(())
}
