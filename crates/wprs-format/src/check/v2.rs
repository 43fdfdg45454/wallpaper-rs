use super::apply;
use crate::meta::{Content, Credit, Links};
use crate::names::safe;
use crate::origin::Origin;
use crate::{Display, Manifest, Playback, Result};
use std::collections::HashSet;
use wprs_scene::Control;

impl Manifest {
    pub(super) fn check_v2(&self) -> Result<()> {
        let (content, origin) = (self.content.as_ref(), self.origin.as_ref());
        let credits = self.credits.len() <= 64 && self.credits.iter().all(Credit::valid);
        let mut lower = HashSet::new();
        let unique = self.properties.keys().all(|id| lower.insert(id.to_lowercase()));
        let combo = |c: &Control| matches!(c, Control::Combo { options, .. } if options.len() > 64);
        let options = !self.properties.values().any(|p| combo(&p.control));
        let accept = |c: &Control| match c {
            Control::File { accept, .. } | Control::Folder { accept, .. } => {
                accept.iter().all(|a| ["image", "video"].contains(&a.as_str()))
            }
            _ => true,
        };
        let accepted = self.properties.values().all(|p| accept(&p.control));
        let limits = self.properties.len() <= 256 && self.presets.len() <= 64;
        let plain = self.authors.iter().chain(&self.tags).all(|t| safe(t));
        if let Some(missing) = self.missing().into_iter().next() {
            return Err(missing);
        }
        apply(&[
            ("authors", plain, "authors and tags have no < or >"),
            (
                "content.descriptors",
                content.is_none_or(Content::valid),
                "lists each reason once, one at least for mature",
            ),
            (
                "origin.derived_from",
                origin.is_none_or(Origin::valid),
                "derived packages credit 1 to 64 works by author without < or >, original ones none",
            ),
            (
                "origin.provider",
                origin.is_none_or(Origin::sourced),
                "provider packages name their provider with a name of [a-z0-9-], https:// addresses and a \
                credit without < or >; others name none",
            ),
            ("preview", self.preview.is_none(), "is called previews.thumbnail in wallpaper-rs.wpak/2"),
            (
                "credits",
                credits,
                "at most 64, each with 1 to 256 paths, an SPDX license and an attribution without < or >",
            ),
            (
                "links",
                self.links.as_ref().is_none_or(Links::valid),
                "expected https:// addresses of 512 characters at most",
            ),
            ("properties", limits, "at most 256 properties and 64 presets"),
            ("properties", accepted, "files and folders accept image, video or both"),
            (
                "playback",
                self.playback.as_ref().is_none_or(Playback::valid),
                "speed is 0.1 to 4, start and end 0 to 3600 s with end after start, volume 0 to 1, max_fps 1 to 240",
            ),
            (
                "display",
                self.display.as_ref().is_none_or(Display::valid),
                "zoom is 1 to 8, position two numbers of -1 to 1 and colors #rrggbb",
            ),
            (
                "properties",
                unique && options,
                "ids differ in more than case and combos have 64 options at most",
            ),
        ])?;
        self.check_texts()
    }
}
