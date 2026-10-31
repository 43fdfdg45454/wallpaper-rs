const report = (name, value) => console.log(`probe ${name} ${JSON.stringify(value)}`);
const watch = (name, element) => {
  element.addEventListener("error", () => report(name, ["error", element.error && element.error.code]));
  const done = () => report(name, ["played", element.muted, element.volume]);
  element.addEventListener("ended", done, { once: true });
  element.addEventListener("timeupdate", () => element.currentTime > 0.3 && done(), { once: false });
  element.play().catch((e) => report(name, ["refused", e.name]));
};
watch("audio", new Audio("tone.wav"));
const video = document.createElement("video");
video.src = "clip.webm";
document.body.append(video);
watch("video", video);
const context = new AudioContext();
const tone = context.createOscillator();
tone.connect(context.destination);
tone.start();
setTimeout(() => report("context", [context.state, context.destination.gain ? context.destination.gain.value : null]), 500);
wprs.onSettings((s) => report("volume", [s.volume, context.destination.gain ? context.destination.gain.value : null]));
