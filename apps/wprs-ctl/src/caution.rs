use crate::input::confirm;
use anyhow::Result;
use wprs_client::{Client, error_name};
use wprs_core::dbus::error::UNCONFIRMED;

pub const FLASHING: &str = "Warning: this wallpaper flashes or shows fast patterns that can trigger seizures \
    in people with photosensitive epilepsy.";

pub async fn apply(client: &Client, uid: &str, output: &str, fit: &str, preset: &str) -> Result<()> {
    let lib = &client.library;
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
    }?;
    let details: wprs_core::model::ItemDetails = wprs_client::parse(&lib.get_item(uid).await?)?;
    let exposes = details.item.package.map(|p| p.exposes).unwrap_or_default();
    if let Some(notice) = wprs_format::web_notice(&exposes) {
        eprintln!("{}\n{RESTRICT} {uid}", notice.local());
    }
    Ok(())
}

pub const RESTRICT: &str =
    "Restrict them with: wprs-ctl lib confirm hide_zone|hide_language|hide_gpu|hide_storage";
