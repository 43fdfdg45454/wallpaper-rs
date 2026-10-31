#[test]
fn upgrades_remote_http_only() {
    assert_eq!(super::secure("http://apod.nasa.gov/a.jpg"), "https://apod.nasa.gov/a.jpg");
    assert_eq!(super::secure("http://127.0.0.1:8080/a.jpg"), "http://127.0.0.1:8080/a.jpg");
    assert_eq!(super::secure("http://[::1]/a.jpg"), "http://[::1]/a.jpg");
    assert_eq!(super::secure("https://example.org/a.jpg"), "https://example.org/a.jpg");
}

#[test]
fn credentials_need_tls_or_loopback() {
    assert!(super::guarded("http://example.org/dav", false).is_err());
    assert!(super::guarded("http://example.org/dav", true).is_ok());
    assert!(super::guarded("http://localhost:2283", false).is_ok());
    assert!(super::guarded("https://example.org/dav", false).is_ok());
}

#[test]
fn secrets_stay_out_of_debug() {
    let text = format!("{:?}", super::Secret::from("hunter2"));
    assert!(!text.contains("hunter2"));
    let header = super::HeaderValue::try_from(&super::Secret::from("hunter2")).unwrap();
    assert!(header.is_sensitive() && !format!("{header:?}").contains("hunter2"));
}

#[test]
fn redirects_never_leave_https() {
    let url = |text: &str| reqwest::Url::parse(text).unwrap();
    let (secure, plain) =
        (url("https://repo.example.org/entry/latest"), url("http://127.0.0.1:8080/entry/latest"));
    let known =
        std::collections::HashSet::from([super::fetch::origin(&plain), super::fetch::origin(&secure)]);
    assert!(super::fetch::follows(&known, std::slice::from_ref(&secure), &secure));
    assert!(super::fetch::follows(&known, std::slice::from_ref(&plain), &plain));
    assert!(!super::fetch::follows(&known, std::slice::from_ref(&secure), &plain));
    assert!(super::refused(&[secure], &plain, false));
    assert!(!super::refused(&[url("http://127.0.0.1/a")], &url("https://repo.example.org/"), false));
}
