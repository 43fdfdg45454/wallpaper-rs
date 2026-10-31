package io.github.wallpaper_rs

const val PROBE_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.probe", "version": "1.0.0",
    "title": "Probe", "license": "CC0-1.0", "kind": "web", "entry": "index.html", "origin": { "kind": "original" },
    "content": { "rating": "general" }, "photosensitive": false, "inputs": ["pointer", "clicks"],
    "web": { "preload": "early.js", "background": "#102030", "storage_kib": 64 },
    "properties": {
        "tint": { "type": "color", "default": "#ff8000", "label": "Tint" },
        "port": { "type": "slider", "min": 1, "max": 65535, "step": 1, "default": 9, "label": "Port" },
        "photo": { "type": "file", "accept": ["image"], "label": "Photo" }
    } }"""

const val PROBE_PAGE = """<!doctype html>
<html><body style="margin:0;height:100vh"><script src="probe.js"></script></body></html>"""

const val PROBE_SCRIPT = """
const probe = (window.probe = { frames: 0, rates: [], paused: [], clicks: [], moves: 0, props: {} });
probe.early = window.early;
probe.state = document.visibilityState;
document.addEventListener("visibilitychange", () => { probe.state = document.visibilityState; });
probe.zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
probe.hour = new Date(Date.UTC(2024, 0, 1, 12)).getHours();
probe.language = navigator.language;
probe.peer = typeof RTCPeerConnection;
const gl = document.createElement("canvas").getContext("webgl");
const info = gl && gl.getExtension("WEBGL_debug_renderer_info");
probe.webgl = gl ? String(gl.getParameter(info ? info.UNMASKED_RENDERER_WEBGL : gl.RENDERER)) : null;
const zoneOf = (g) => new g.Intl.DateTimeFormat().resolvedOptions().timeZone;
document.body.append(Object.assign(document.createElement("iframe"), { hidden: true }));
const inner = window[0];
probe.routes = {
  frame: zoneOf(inner), frameHour: new inner.Date(Date.UTC(2024, 0, 1, 12)).getHours(), frameLanguage: inner.navigator.language,
  framePeer: typeof inner.RTCPeerConnection, made: new Intl.DateTimeFormat.prototype.constructor().resolvedOptions().timeZone,
  madeHour: new (Object.getPrototypeOf(new Date()).constructor)(Date.UTC(2024, 0, 1, 12)).getHours(),
};
const code = "postMessage([new Intl.DateTimeFormat().resolvedOptions().timeZone, new Date(Date.UTC(2024, 0, 1, 12)).getHours(), navigator.language])";
const worker = new Worker(URL.createObjectURL(new Blob([code], { type: "text/javascript" })));
worker.onmessage = (event) => { probe.worker = event.data; };
worker.onerror = (event) => { probe.worker = ["error", String(event.message)]; };
wprs.onProperties((changed) => {
  Object.assign(probe.props, changed);
  if (changed.tint) document.body.style.background = changed.tint.hex;
});
wprs.onSettings((settings) => { probe.settings = settings; });
wprs.onPause((paused) => probe.paused.push(paused));
addEventListener("click", (event) => probe.clicks.push(event.isTrusted));
addEventListener("pointermove", () => { probe.moves += 1; });
addEventListener("pointerdown", (event) => { probe.downs = (probe.downs || 0) + 1; });
const reach = (name, url) => fetch(url).then((r) => (r.ok ? r.text() : Promise.reject(r.status)))
  .then((text) => { probe[name] = "read " + text.length; }, () => { probe[name] = "blocked"; });
reach("own", "probe.js");
reach("manifest", "manifest.json");
reach("escape", "../manifest.json");
reach("other", "https://0000000000000000.wpak.invalid/index.html");
reach("file", "file:///system/etc/hosts");
reach("wpak", "wpak://org.example.probe/index.html");
reach("local", "http://127.0.0.1:" + wprs.properties.port + "/");
if (wprs.properties.photo) reach("photo", wprs.properties.photo);
const count = () => { probe.frames += 1; requestAnimationFrame(count); };
requestAnimationFrame(count);
setInterval(() => { probe.rates.push(probe.frames); probe.frames = 0; }, 1000);
try {
  const visits = Number(localStorage.getItem("visits") || 0) + 1;
  localStorage.setItem("visits", String(visits));
  probe.visits = visits;
} catch (e) {
  probe.visits = String(e);
}
"""
