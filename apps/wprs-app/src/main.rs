mod app;
#[macro_use]
mod args;
mod backend;
mod bus;
mod calls;
mod diff;
mod events;
mod fetch;
mod grid;
mod items;
mod library;
mod rows;
mod secret;
#[cfg(test)]
mod tests;
mod tr;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use wprs_core::dbus::APP_ID;

const MAIN: &str = "qrc:/qt/qml/io/github/wallpaper_rs/app/qml/Main.qml";
const STYLE: &str = "org.kde.desktop";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => println!("wprs-app {}", env!("CARGO_PKG_VERSION")),
        Some("--help" | "-h") => println!("usage: wprs-app [--check | --version] [FILE | URI]..."),
        Some("--check") if !bus::checking() => return check(),
        _ => return app::start(args.iter().filter_map(|a| file(a)).collect()).unwrap_or_else(run),
    }
    ExitCode::SUCCESS
}

fn file(arg: &str) -> Option<String> {
    if arg.contains("://") {
        return Some(arg.to_owned());
    }
    let path = std::path::absolute(arg).ok().filter(|p| p.is_file())?;
    Some(bus::url(path.to_str()?))
}

fn run() -> ExitCode {
    let mut app = QGuiApplication::new();
    QGuiApplication::set_desktop_file_name(&QString::from(APP_ID));
    let mut engine = QQmlApplicationEngine::new();
    let (Some(mut app), Some(mut engine)) = (app.as_mut(), engine.as_mut()) else { return ExitCode::FAILURE };
    app.as_mut().set_application_name(&QString::from("wallpaper-rs"));
    engine.as_mut().add_import_path(&QString::from("qrc:/qt/qml"));
    let paths = engine.import_path_list();
    let style = paths.iter().any(|p| Path::new(&p.to_string()).join(STYLE.replace('.', "/")).is_dir());
    if style && std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&QString::from(STYLE));
    }
    if bus::checking() {
        engine.as_mut().on_object_creation_failed(|_, _| std::process::exit(1)).release();
    }
    engine.load(&QUrl::from(MAIN));
    ExitCode::from(app.exec() as u8)
}

fn check() -> ExitCode {
    let platform = std::env::var("QT_QPA_PLATFORM").unwrap_or_else(|_| "offscreen".into());
    let child = std::env::current_exe().and_then(|exe| {
        let mut command = Command::new(exe);
        command.env(bus::CHECK, "1").env("QT_QPA_PLATFORM", platform).env("QT_FORCE_STDERR_LOGGING", "1");
        command.stderr(Stdio::piped()).spawn()
    });
    let Ok(mut child) = child else { return ExitCode::FAILURE };
    let mut problems = 0;
    for line in BufReader::new(child.stderr.take().unwrap()).lines().map_while(Result::ok) {
        eprintln!("{line}");
        problems +=
            usize::from([".qml", "QQmlApplicationEngine", "call failed"].iter().any(|p| line.contains(p)));
    }
    match child.wait() {
        Ok(status) if status.success() && problems == 0 => ExitCode::SUCCESS,
        _ => {
            eprintln!("wprs-app: the QML check found {problems} problems");
            ExitCode::FAILURE
        }
    }
}
