use std::path::PathBuf;
use wprs_web::{Folder, Link, Start, ToWeb};

#[test]
fn large_messages_travel_whole() {
    let (ours, theirs) = Link::pair().unwrap();
    let theirs = Link::from_fd(theirs);
    let long = "a".repeat(180);
    let folder = |prop: &str| Folder {
        prop: prop.into(),
        all: true,
        files: (0..1000)
            .map(|i| (format!("wpak-user://{prop}/{i:016}.png"), PathBuf::from(format!("/{long}/{i}.png"))))
            .collect(),
    };
    let start =
        ToWeb::Start(Box::new(Start { folders: vec![folder("one"), folder("two")], ..Start::default() }));
    assert!(serde_json::to_vec(&start).unwrap().len() > wprs_web::MAX_MESSAGE);
    ours.send(&start, &[]).unwrap();
    ours.send(&ToWeb::Quit, &[]).unwrap();
    let (got, fds) = theirs.recv::<ToWeb>(true).unwrap().unwrap();
    assert_eq!((got, fds.len()), (start, 0));
    assert_eq!(theirs.recv::<ToWeb>(true).unwrap().unwrap().0, ToWeb::Quit);
}
