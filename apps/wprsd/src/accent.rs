use crate::compositor::Content;
use crate::daemon::Daemon;
use crate::sceneprops::Props;
use std::path::Path;
use wprs_scene::Values;

fn authored(props: &Props, values: &Values) -> Option<u32> {
    props.authored.accent.as_ref()?.resolve(values)
}

impl Daemon {
    pub(crate) fn accent(
        &self,
        output: &str,
        (image, display): (Option<&Path>, &wprs_format::Display),
        content: &Content,
        video: Option<&Props>,
    ) -> Option<String> {
        let shown = match content {
            Content::Scene(s) => authored(&s.pack.props, &s.values),
            Content::Web(p) => authored(&p.pack.props, &p.values),
            Content::Video(_) | Content::Image(_) => {
                let chosen = |p: &Props| p.values(&self.state().config.scene_values(output, &p.id));
                let values = video.map(chosen).unwrap_or_default();
                video.and_then(|p| authored(p, &values)).or_else(|| display.accent.as_ref()?.resolve(&values))
            }
            _ => None,
        };
        let color = shown.or_else(|| self.dominant(image?))?;
        Some(format!("#{:06x}", color & 0xff_ffff))
    }

    fn dominant(&self, image: &Path) -> Option<u32> {
        let found = |lib: &wprs_library::Library| lib.item(&lib.resolve_item(&image.to_string_lossy())?);
        self.store.read(found).ok()?.meta.color
    }
}
