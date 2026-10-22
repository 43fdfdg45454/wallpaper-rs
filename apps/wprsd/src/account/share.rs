use super::keys::public;
use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, invalid};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde_json::json;
use std::sync::Arc;
use wprs_core::model::JobKind;
use wprs_sync::{code, seal_key};

impl Daemon {
    pub fn share_key(self: &Arc<Self>, device: &str, shown: &str) -> Result<u64> {
        let digits: String = shown.chars().filter(char::is_ascii_digit).collect();
        if digits.len() != 12 {
            return Err(invalid("the code has 12 digits"));
        }
        self.require_ready()?;
        let device = device.to_owned();
        Ok(self.start_job(JobKind::Account, &format!("share {device}"), move |d, _| {
            let (api, secrets) = d.account_api(true)?;
            let (mine, peer) = (secrets.device_key()?, d.peer(&api, &device)?);
            let theirs = public(&peer)?;
            if code(&mine.public(), &theirs).replace(' ', "") != digits {
                return Err(invalid(
                    "the codes differ: do not share the key and check which device asks for it",
                ));
            }
            let sealed = seal_key(&mine, &theirs, &d.aad(&api, &peer.id)?, &secrets.sync_key()?)
                .map_err(sync_error)?;
            let body = json!({"to_device": peer.id, "kind": "key-share", "body": B64.encode(sealed)});
            block_on(api.post("/api/v1/mailbox", body)).map_err(sync_error)?;
            Ok(json!({"device": peer.name}))
        }))
    }
}
