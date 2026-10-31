use std::time::Duration;

pub(crate) const PATIENCE_NS: i64 = 2_000_000_000;

pub(crate) fn begins(gone: bool, coming: bool, waited_ns: i64) -> bool {
    (gone && coming) || waited_ns > PATIENCE_NS
}

pub(crate) fn fades(fade: Duration, fps: u32, hidden: bool, from_video: bool) -> bool {
    !fade.is_zero() && fps > 0 && !(hidden && from_video)
}

pub(crate) fn resumes(parked: Option<&str>, chosen: Option<&str>) -> bool {
    parked.is_some() && parked == chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    const FADE: Duration = Duration::from_millis(600);

    #[test]
    fn a_fade_waits_until_both_sides_have_a_frame() {
        assert!(!begins(true, false, 0), "a picture fading into a video waits for the first video frame");
        assert!(!begins(false, true, PATIENCE_NS / 2), "a video fading out waits for its last frame");
        assert!(begins(true, true, 0));
        assert!(
            !begins(true, false, 1_000_000_000),
            "a phone decoder may take a second to give its first frame"
        );
        assert!(begins(true, false, PATIENCE_NS + 1), "a video that never plays does not hold the fade");
    }

    #[test]
    fn a_video_left_while_hidden_is_not_played_again_to_fade_it_out() {
        assert!(fades(FADE, 30, false, true));
        assert!(fades(FADE, 30, true, false), "a picture left while hidden still fades into the video");
        assert!(!fades(FADE, 30, true, true));
        assert!(!fades(Duration::ZERO, 30, false, false) && !fades(FADE, 0, false, false));
    }

    #[test]
    fn a_new_surface_takes_back_only_the_scene_it_shows() {
        assert!(resumes(Some("snow"), Some("snow")));
        assert!(!resumes(Some("snow"), Some("clip")), "another surface's scene would fade into this one");
        assert!(!resumes(None, None) && !resumes(Some("snow"), None));
    }
}
