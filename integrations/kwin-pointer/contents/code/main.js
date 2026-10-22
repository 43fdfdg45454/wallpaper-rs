const timer = new QTimer();
timer.singleShot = true;
timer.interval = 16;
timer.timeout.connect(() => {
    const p = workspace.cursorPos;
    callDBus("io.github.wallpaper_rs.Daemon", "/io/github/wallpaper_rs/Daemon", "io.github.wallpaper_rs.Inputs1",
        "SetPointer", Math.round(p.x), Math.round(p.y));
});
workspace.cursorPosChanged.connect(() => timer.active || timer.start());
timer.start();
