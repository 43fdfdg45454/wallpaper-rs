use super::*;

fn shown(frame: &Frame, i: usize) -> String {
    match &frame.draws[i].content {
        Content::Text { text, .. } => text.to_string(),
        other => panic!("not a text: {other:?}"),
    }
}

fn refused(text: serde_json::Value) -> bool {
    let layers = json!([{ "id": "t", "type": "text", "text": text }]);
    Scene::load(&scene(layers), &props()).is_err()
}

#[test]
fn placeholders_show_while_nothing_plays_in_the_language_of_the_screen() {
    let layers = json!([
        { "id": "title", "type": "text", "text": { "media": "title", "placeholder": { "en": "Nothing plays", "es": "No suena nada" } } },
        { "id": "artist", "type": "text", "text": { "media": "artist", "placeholder": "—" } },
        { "id": "album", "type": "text", "text": { "media": "album" } }
    ]);
    let mut rt = runtime(&scene(layers));
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!([shown(&frame, 0), shown(&frame, 1), shown(&frame, 2)], ["Nothing plays", "—", ""]);
    rt.set_language("es");
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!(shown(&frame, 0), "No suena nada");
    let track = MediaState { enabled: true, track: 1, title: "Night Drive".into(), ..MediaState::default() };
    rt.set_media(track);
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!([shown(&frame, 0), shown(&frame, 1)], ["Night Drive", ""], "a track shows its own texts");
    rt.set_media(MediaState::default());
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!(shown(&frame, 0), "No suena nada", "the placeholder comes back when the track ends");
}

#[test]
fn placeholders_are_texts_of_media_texts_only() {
    assert!(!refused(json!({ "media": "title", "placeholder": "" })));
    assert!(refused(json!({ "media": "progress", "placeholder": "0%" })), "only texts of the track");
    assert!(refused(json!({ "prop": "style", "placeholder": "x" })), "only with the media player");
    assert!(refused(json!({ "media": "title", "placeholder": "x".repeat(513) })));
    assert!(!refused(json!({ "media": "title", "placeholder": "x".repeat(512) })));
    assert!(refused(json!({ "media": "title", "placeholder": {} })));
    assert!(refused(json!({ "media": "title", "placeholder": { "not a language": "x" } })));
    assert!(refused(json!({ "media": "title", "placeholder": { "pt-BR": "y".repeat(600) } })));
    assert!(refused(json!({ "media": "title", "placeholder": 5 })));
    let solid =
        json!([{ "id": "s", "type": "solid", "opacity": { "media": "progress", "placeholder": "x" } }]);
    assert!(Scene::load(&scene(solid), &props()).is_err(), "a number has no placeholder");
}
