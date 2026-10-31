use super::*;
use wprs_library::{Error, query_keys};

#[test]
fn every_key_filters_items() {
    let Fixture { library, source, .. } = fixture();
    let source_query = format!("source:{source}");
    let cases: [(&str, &[&str]); 49] = [
        ("", &["café", "forest", "night", "ocean waves", "square"]),
        ("waves", &["ocean waves"]),
        ("oce wav", &["ocean waves"]),
        ("\"ocean waves\"", &["ocean waves"]),
        ("\"waves ocean\"", &[]),
        ("-waves", &["café", "forest", "night", "square"]),
        ("tag:sea", &["night", "ocean waves"]),
        ("tag:SEA tag:blue", &["ocean waves"]),
        ("-tag:sea", &["café", "forest", "square"]),
        ("tag:\"sea\"", &["night", "ocean waves"]),
        ("kind:video", &["forest", "night"]),
        ("kind:image", &["café", "ocean waves", "square"]),
        ("kind:package", &[]),
        ("kind:scene", &[]),
        ("kind:web", &[]),
        ("fav:", &["café"]),
        ("fav:no", &["forest", "night", "ocean waves", "square"]),
        ("rating:>=4", &["café", "night"]),
        ("rating:5", &["night"]),
        ("rating:=4", &["café"]),
        ("rating:>4", &["night"]),
        ("rating:<3", &["forest", "ocean waves", "square"]),
        ("rating:<=2", &["forest", "ocean waves", "square"]),
        ("res:4k", &["café"]),
        ("res:1440p", &["café", "forest"]),
        ("res:>=1920x1080", &["café", "forest", "ocean waves"]),
        ("res:<1920x1080", &["square"]),
        ("orient:portrait", &["night"]),
        ("orient:landscape", &["café", "forest", "ocean waves"]),
        ("orient:square", &["square"]),
        ("dur:<30s", &["night"]),
        ("dur:>1m", &["forest"]),
        ("dur:<=2m", &["forest", "night"]),
        ("audio:yes", &["night"]),
        ("audio:no", &["forest"]),
        (&source_query, &["café", "night", "ocean waves", "square"]),
        ("in:Picks", &["forest", "ocean waves"]),
        ("in:picks -kind:video", &["ocean waves"]),
        ("added:<7d", &["forest"]),
        ("added:>7d", &["café", "night", "ocean waves", "square"]),
        ("used:never", &["café", "night", "ocean waves", "square"]),
        ("used:<1h", &["forest"]),
        ("missing:", &["square"]),
        ("-missing:", &["café", "forest", "night", "ocean waves"]),
        ("color:#3366ff", &["café"]),
        ("color:#3366f0", &["café"]),
        ("color:#000000", &["night", "ocean waves"]),
        ("color:#ff0000", &[]),
        ("Kind:VIDEO -tag:sea", &["forest"]),
    ];
    for (query, expected) in cases {
        assert_eq!(titles(&library, query), expected, "{query}");
        assert_eq!(library.count(query, NOW).unwrap() as usize, expected.len(), "{query}");
    }
}

#[test]
fn free_words_ignore_diacritics_and_match_prefixes_of_titles_tags_and_paths() {
    let Fixture { library, .. } = fixture();
    for query in ["cafe", "CAFÉ", "caf", "warm", "pictures café", "jpg"] {
        assert_eq!(titles(&library, query), ["café"], "{query}");
    }
    assert_eq!(titles(&library, "loose"), ["forest"]);
}

#[test]
fn injection_looking_input_is_only_ever_data() {
    let Fixture { library, .. } = fixture();
    let attempts = [
        "tag:'); DROP TABLE items;--",
        "\"'); DROP TABLE items;--\"",
        "in:x' OR '1'='1",
        "source:\"x' OR 1=1 --\"",
        "tag:\"sea' OR 'a'='a\"",
        "'; DELETE FROM items; --",
        "NEAR(a b) OR x*",
    ];
    for query in attempts {
        assert_eq!(titles(&library, query), Vec::<String>::new(), "{query}");
    }
    assert_eq!(library.count("", NOW).unwrap(), 5);
    assert_eq!(library.tags().unwrap().len(), 3);
}

#[test]
fn unknown_keys_and_bad_values_are_clear_errors() {
    let Fixture { library, .. } = fixture();
    let error = library.search("size:big", NOW, 0, 10).unwrap_err();
    assert!(
        error.to_string().starts_with("invalid query: unknown key \"size:\", expected one of tag, kind"),
        "{error}"
    );
    let error = library.count("rating:9", NOW).unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid query: invalid value in \"rating:9\", expected a rating from 0 to 5 with an optional >=, <=, >, < or ="
    );
    let bad = [
        "kind:gif",
        "res:big",
        "dur:soon",
        "color:blue",
        "color:#12345",
        "orient:diagonal",
        "audio:",
        "tag:",
        "fav:maybe",
        "added:7",
        "sort:size",
        "-sort:title",
        "\"unclosed",
    ];
    for query in bad {
        assert!(matches!(library.search(query, NOW, 0, 10), Err(Error::Query(_))), "{query}");
    }
}

#[test]
fn query_keys_describe_every_key() {
    let Fixture { library, .. } = fixture();
    let keys = query_keys();
    assert_eq!(keys.len(), 18);
    assert!(keys.contains(&("content", "general, mature or unrated")));
    assert!(keys.contains(&("in", "a manual playlist uid or name")));
    assert!(keys.contains(&(
        "sort",
        "added, title, rating, used, random or random:<seed>, with a - suffix for descending"
    )));
    for (key, _) in keys {
        let unknown =
            library.count(&format!("{key}:%"), NOW).is_err_and(|e| e.to_string().contains("unknown key"));
        assert!(!unknown, "{key}");
    }
}

#[test]
fn uids_pick_items_whatever_else_the_query_asks() {
    let Fixture { library, .. } = fixture();
    let all = library.search("", NOW, 0, 10).unwrap();
    let [first, second] = [&all[0].uid, &all[1].uid];
    assert_eq!(titles(&library, &format!("uid:{first},{second}")).len(), 2);
    assert_eq!(library.count(&format!("uid:{first},,{second},NONE"), NOW).unwrap(), 2);
    let title = &all[0].title;
    assert_eq!(library.count(&format!("-uid:{first} \"{title}\""), NOW).unwrap(), 0);
    let many = vec!["X"; 501].join(",");
    assert!(matches!(library.count(&format!("uid:{many}"), NOW), Err(Error::Query(_))));
    assert!(matches!(library.count("uid:", NOW), Err(Error::Query(_))));
}
