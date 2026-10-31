use crate::Error;
use crate::said;
use crate::state::{api, linked, read, update};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use wprs_sync::{Api, Peer, devices};

pub(crate) async fn refresh(api: &Api) -> Result<Vec<Peer>, Error> {
    let peers = devices(api).await?;
    update(|s| {
        s.pinned.iter_mut().for_each(|pins| pins.retain(|id, _| peers.iter().any(|p| p.id == *id)));
        s.members.extend(peers.iter().map(|p| p.id.clone()));
        s.peers = peers.clone();
    })?;
    Ok(peers)
}

pub(crate) async fn peer(api: &Api, wanted: &str) -> Result<Peer, Error> {
    let (peers, wanted) = (refresh(api).await?, wanted.trim());
    let matches = |p: &&Peer| {
        p.id == wanted
            || p.name.eq_ignore_ascii_case(wanted)
            || read(|s| s.label_of(&p.id)).is_some_and(|l| l == wanted)
    };
    let peer = peers
        .iter()
        .find(matches)
        .ok_or_else(|| said("android.no_device", "no device %1 on the server", &[&wanted]))?;
    match read(|s| s.device.as_ref() == Some(&peer.id)) {
        true => Err(said("android.choose_another_device", "choose another device than this phone", &[])),
        false => Ok(peer.clone()),
    }
}

pub(crate) fn pins() -> Result<BTreeMap<String, String>, Error> {
    let seeded = read(|s| {
        if let Some(pins) = &s.pinned {
            return Ok(pins.clone());
        }
        let record = |id: &str| s.replica.bases.get(&format!("device/{id}")).and_then(|(_, doc)| doc.view());
        let vouched = |p: &&Peer| {
            record(&p.id).and_then(|v| v.get("key").cloned()) == p.enc_key.clone().map(Value::from)
        };
        let peers = s.peers.iter().filter(|p| Some(&p.id) != s.device.as_ref()).filter(vouched);
        Err(peers.filter_map(|p| Some((p.id.clone(), p.enc_key.clone()?))).collect::<BTreeMap<_, _>>())
    });
    seeded.or_else(|seeded| update(|s| s.pinned = Some(seeded.clone())).map(|()| seeded))
}

pub(crate) fn pin(peer: &str, key: &str) -> Result<(), Error> {
    update(|s| drop(s.pinned.get_or_insert_default().insert(peer.to_owned(), key.to_owned())))
}

pub(crate) async fn rekeyed(api: &Api) -> Result<bool, Error> {
    let before = read(|s| s.members.clone());
    let peers = refresh(api).await?;
    let left = before.iter().any(|id| !peers.iter().any(|p| p.id == *id));
    update(|s| s.members = peers.iter().map(|p| p.id.clone()).collect::<BTreeSet<_>>())?;
    Ok(left)
}

pub(crate) async fn device_code(code: &str, action: &str, mut body: Value) -> Result<Value, Error> {
    let letters: String = code.chars().filter(char::is_ascii_alphanumeric).collect();
    if letters.len() != 8 {
        return Err(said("android.give_letters_shown", "give the 8 letters shown on the new device", &[]));
    }
    linked()?;
    body["user_code"] = json!(letters.to_uppercase());
    let (api, _) = api()?;
    Ok(api.post(&format!("/api/v1/auth/device/{action}"), body).await?)
}
