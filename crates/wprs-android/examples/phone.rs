use std::io::{BufRead, Write};
use wprs_android::host;

fn main() {
    let dir = std::env::args().nth(1).expect("usage: phone <files dir>, then one request per line on stdin");
    host::prepare(&dir).expect("the files dir cannot be used");
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let (area, rest) = line.split_once(' ').unwrap_or((&line, ""));
        if area == "account" && rest.contains("\"listen\"") {
            let rest = rest.to_owned();
            std::thread::spawn(move || println!("{}", host::account(&rest)));
            continue;
        }
        let answer = match area {
            "account" => host::account(rest),
            "library" => host::library(rest),
            "depth" => host::depth(rest),
            "import" => host::import(rest),
            "setting" => {
                let (key, value) = rest.split_once(' ').map_or((rest, None), |(k, v)| (k, Some(v)));
                host::setting(key, value)
            }
            _ => format!("{{\"error\": \"unknown area {area}\"}}"),
        };
        println!("{answer}");
        std::io::stdout().flush().ok();
    }
}
