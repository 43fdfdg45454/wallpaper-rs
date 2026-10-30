use super::*;

pub(super) fn accepted_shaders_run_the_helper_once(book: &Path) {
    let runs = helper_runs();
    let (d, first) = timed(|| Descriptor::packaged("h", &json("h"), GOOD).unwrap());
    let (_, cached) = timed(|| Descriptor::packaged("h", &json("h"), GOOD).unwrap());
    println!("an accepted shader took {first:?} with the helper and {cached:?} from the cache");
    assert_eq!(helper_runs(), runs + 1);
    guarded(&d).unwrap();
    let kept = lines(&book.with_file_name("checked-effects"));
    assert!(kept.len() == 1 && kept[0].ends_with(" ok") && kept[0].len() == 67, "{kept:?}");
    let forged = Descriptor { key: d.key ^ 2, ..d };
    assert_eq!(guarded(&forged).unwrap_err(), "its shader was not checked");
}

pub(super) fn refused_shaders_return_the_message_of_the_helper(book: &Path) {
    set_checker(None);
    let inside = Descriptor::packaged("h", &json("h"), ENDLESS).unwrap_err();
    checker(|_| ());
    let runs = helper_runs();
    assert_eq!(Descriptor::packaged("h", &json("h"), ENDLESS).unwrap_err(), inside);
    assert_eq!(Descriptor::packaged("h", &json("h"), ENDLESS).unwrap_err(), inside);
    assert_eq!(helper_runs(), runs + 1);
    let kept = lines(&book.with_file_name("checked-effects"));
    let quoted = serde_json::Value::from(inside.as_str()).to_string();
    assert!(kept.len() == 2 && kept[1].ends_with(&format!(" refused {quoted}")), "{kept:?}");
}

pub(super) fn the_disk_cache_survives_a_new_process(book: &Path) {
    let me = std::env::current_exe().unwrap();
    let out = Command::new(me).arg("--again").arg(book).output().unwrap();
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && said.trim() == "runs 0",
        "{said}{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

pub(super) fn again(book: &str) {
    keep_strikes(Path::new(book));
    check_out_of_process(true);
    Descriptor::packaged("h", &json("h"), GOOD).unwrap();
    assert!(Descriptor::packaged("h", &json("h"), ENDLESS).is_err());
    println!("runs {}", helper_runs());
}
