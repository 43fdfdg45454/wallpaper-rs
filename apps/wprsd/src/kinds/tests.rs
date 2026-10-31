use super::*;

fn options(pairs: &[(&str, &str)]) -> Options {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn refused(kind: &str, pairs: &[(&str, &str)]) -> String {
    effective(&kind_of(kind).unwrap(), &options(pairs)).unwrap_err().to_string()
}

#[test]
fn options_are_validated_and_completed_with_defaults() {
    let bing = effective(&kind_of("bing").unwrap(), &options(&[("count", "3")])).unwrap();
    assert_eq!(
        bing,
        options(&[
            ("cache_mb", "1024"),
            ("count", "3"),
            ("market", "en-US"),
            ("refresh", "12h"),
            ("uhd", "true")
        ])
    );
    let immich = options(&[("server", "http://127.0.0.1:2283"), ("album", "0d6a-11"), ("refresh", "0s")]);
    assert_eq!(effective(&kind_of("immich").unwrap(), &immich).unwrap()["allow_plain_http"], "false");
    assert!(refused("bing", &[("count", "9")]).contains("from 1 to 8"));
    assert!(refused("bing", &[("refresh", "5m")]).contains("15m to 30d"));
    assert!(refused("bing", &[("password", "x")]).contains("is a secret"));
    assert!(refused("bing", &[("base", "x")]).contains("no option base"));
    assert!(refused("webdav", &[("url", "https://example.org/dav")]).contains("needs the option user"));
    assert!(refused("wallhaven", &[("purity", "123")]).contains("three digits"));
    for url in ["https://me:pw@example.org/dav", "https://example.org/dav?token=1", "https://example.org/#x"]
    {
        assert!(refused("webdav", &[("url", url), ("user", "me")]).contains("not in the URL"), "{url}");
    }
    assert!(refused("webdav", &[("url", "http://example.org/dav"), ("user", "me")]).contains("plain http"));
    let plain = [("url", "http://example.org/dav"), ("user", "me"), ("allow_plain_http", "true")];
    assert!(effective(&kind_of("webdav").unwrap(), &options(&plain)).is_ok());
    assert!(kind_of("local").is_err() && check_id("Photos").is_err() && check_id("photos-2_x").is_ok());
}

#[test]
fn entries_round_trip_through_the_configuration() {
    let kind = kind_of("nextcloud").unwrap();
    let given = options(&[("server", "https://cloud.example.org"), ("user", "me"), ("cache_mb", "64")]);
    let table = table(&kind, &given);
    assert_eq!(table.get("cache_mb").and_then(toml::Value::as_integer), Some(64));
    let (known, read, options) = entry(&table).unwrap();
    assert_eq!((known.kind.as_str(), read, options["folder"].as_str()), ("nextcloud", given, ""));
    assert_eq!(hosts(&kind, &options), ["cloud.example.org"]);
    let mut written = table.clone();
    written.as_table_mut().unwrap().insert("password".into(), "hunter2".into());
    let error = entry(&written).unwrap_err();
    assert!(error.contains("is a secret") && !error.contains("hunter2"), "{error}");
}
