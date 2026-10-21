use crate::playlist::{Order, Playlist};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistState {
    seed: u32,
    cycle: u32,
    position: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    since: Option<DateTime<Utc>>,
}

impl PlaylistState {
    pub fn new(seed: u32) -> Self {
        Self { seed, cycle: 0, position: 0, previous: None, since: None }
    }

    pub fn index(&self, playlist: &Playlist) -> Option<usize> {
        let len = playlist.items.len();
        (len > 0).then(|| match playlist.order {
            Order::Sequential => self.position % len,
            Order::Shuffle => self.permutation(len)[self.position % len],
        })
    }

    pub fn current<'a>(&self, playlist: &'a Playlist) -> Option<&'a Path> {
        self.index(playlist).map(|i| playlist.items[i].as_path())
    }

    pub fn advance<'a>(&mut self, playlist: &'a Playlist, now: DateTime<Utc>) -> Option<&'a Path> {
        if self.position + 1 >= playlist.items.len() {
            self.previous = self.index(playlist);
            self.cycle = self.cycle.wrapping_add(1);
            self.position = 0;
        } else {
            self.position += 1;
        }
        self.since = Some(now);
        self.current(playlist)
    }

    pub fn poll<'a>(&mut self, playlist: &'a Playlist, now: DateTime<Utc>) -> Option<&'a Path> {
        self.since.get_or_insert(now);
        match self.next_switch(playlist) {
            Some(due) if now >= due => self.advance(playlist, now),
            _ => self.current(playlist),
        }
    }

    pub fn next_switch(&self, playlist: &Playlist) -> Option<DateTime<Utc>> {
        let interval = TimeDelta::from_std(playlist.interval).ok().filter(|d| !d.is_zero())?;
        self.since.map(|since| since + interval)
    }

    fn permutation(&self, len: usize) -> Vec<usize> {
        let mut x = mix(u64::from(self.seed) << 32 | u64::from(self.cycle)) | 1;
        let mut order: Vec<usize> = (0..len).collect();
        for i in (1..len).rev() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            order.swap(i, ((u128::from(x) * (i as u128 + 1)) >> 64) as usize);
        }
        if len > 1 && order.first() == self.previous.as_ref() {
            order.swap(0, len - 1);
        }
        order
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Positions(pub BTreeMap<String, PlaylistState>);

impl Positions {
    pub fn state(&mut self, playlist: &Playlist, output: &str, seed: u32) -> &mut PlaylistState {
        let key = if playlist.per_output { format!("{}/{output}", playlist.id) } else { playlist.id.clone() };
        let hash = key.bytes().fold(0x811c_9dc5_u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
        self.0.entry(key).or_insert_with(|| PlaylistState::new(seed ^ hash))
    }
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
