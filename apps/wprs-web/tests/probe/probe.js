const report = (name, value) => console.log(`probe ${name} ${JSON.stringify(value)}`);
const canvas = document.getElementById("c");
canvas.width = innerWidth;
canvas.height = innerHeight;
const draw = canvas.getContext("2d");
let frames = 0;
const tick = () => {
  frames += 1;
  draw.fillStyle = `hsl(${frames % 360}, 80%, 50%)`;
  draw.fillRect(0, 0, canvas.width, canvas.height);
  requestAnimationFrame(tick);
};
requestAnimationFrame(tick);
setInterval(() => { report("frames", frames); frames = 0; }, 1000);
report("early", window.early);
report("zone", Intl.DateTimeFormat().resolvedOptions().timeZone);
report("language", navigator.language);
const gl = document.createElement("canvas").getContext("webgl");
const info = gl && gl.getExtension("WEBGL_debug_renderer_info");
report("webgl", gl ? String(gl.getParameter(info ? info.UNMASKED_RENDERER_WEBGL : gl.RENDERER)) : null);
wprs.onProperties((changed) => report("properties", changed));
wprs.onSettings((settings) => report("settings", settings));
wprs.onPause((paused) => report("paused", paused));
wprs.onAudio((bands) => report("audio", bands.left.length));
addEventListener("pointermove", (e) => report("move", [Math.round(e.clientX), Math.round(e.clientY), e.isTrusted]));
addEventListener("click", (e) => report("click", [Math.round(e.clientX), Math.round(e.clientY), e.isTrusted]));
const reach = (name, url) => fetch(url).then((r) => r.text()).then((t) => report(name, `read ${t.length}`), () => report(name, "blocked"));
reach("own", "probe.js");
reach("manifest", "manifest.json");
reach("other", "wpak://org.example.other/index.html");
reach("file", "file:///etc/passwd");
reach("escape", "wpak://org.example.probe/../../etc/passwd");
reach("user", "wpak-user://photos/0000000000000000.png");
reach("local", `http://127.0.0.1:${wprs.properties.port || 9}/`);
wprs.files("photos").then((list) => report("files", list));
wprs.randomFile("photos").then((one) => one && reach("picked", one));
wprs.onFiles("photos", (change) => report("change", change));
try {
  localStorage.setItem("visits", String(Number(localStorage.getItem("visits") || 0) + 1));
  report("visits", localStorage.getItem("visits"));
} catch (e) {
  report("visits", String(e));
}
