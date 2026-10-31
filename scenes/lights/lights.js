const canvas = document.getElementById("lights");
const draw = canvas.getContext("2d");
const photo = document.getElementById("photo");
const clock = document.getElementById("clock");
const saved = JSON.parse(localStorage.getItem("lights") || "[]");
let props = { color: { r: 0.5, g: 0.83, b: 1 }, count: 16, speed: 1, clock: true };
let lights = [];
let level = 0;
let pointer = null;
let running = true;
let last = performance.now();

const resize = () => {
  canvas.width = innerWidth;
  canvas.height = innerHeight;
};

const spawn = (i) => saved[i] || { x: Math.random(), y: Math.random(), a: Math.random() * 6.28, r: 0.04 + Math.random() * 0.08 };

const fill = () => {
  lights = Array.from({ length: props.count }, (_, i) => lights[i] || spawn(i));
};

const time = () => {
  const now = new Intl.DateTimeFormat(wprs.locale || undefined, { hour: "2-digit", minute: "2-digit" });
  clock.textContent = props.clock ? now.format(new Date()) : "";
};

const step = (now) => {
  if (!running) return;
  const dt = Math.min(0.1, (now - last) / 1000);
  last = now;
  const { r, g, b } = props.color;
  draw.clearRect(0, 0, canvas.width, canvas.height);
  draw.globalCompositeOperation = "lighter";
  for (const light of lights) {
    light.a += dt * props.speed * 0.3;
    light.x = (light.x + Math.cos(light.a) * dt * props.speed * 0.02 + 1) % 1;
    light.y = (light.y + Math.sin(light.a * 0.7) * dt * props.speed * 0.02 + 1) % 1;
    if (pointer) {
      const dx = light.x - pointer.x, dy = light.y - pointer.y, d = Math.hypot(dx, dy) + 0.01;
      if (d < 0.2) { light.x += (dx / d) * dt * 0.1; light.y += (dy / d) * dt * 0.1; }
    }
    const size = light.r * Math.min(canvas.width, canvas.height) * (1 + level);
    const x = light.x * canvas.width, y = light.y * canvas.height;
    const glow = draw.createRadialGradient(x, y, 0, x, y, size);
    glow.addColorStop(0, `rgba(${r * 255 | 0}, ${g * 255 | 0}, ${b * 255 | 0}, 0.5)`);
    glow.addColorStop(1, "rgba(0, 0, 0, 0)");
    draw.fillStyle = glow;
    draw.fillRect(x - size, y - size, size * 2, size * 2);
  }
  requestAnimationFrame(step);
};

const show = async () => {
  const picked = await wprs.randomFile("photos");
  if (!picked) return photo.classList.remove("shown");
  photo.onload = () => photo.classList.add("shown");
  photo.src = picked;
};

addEventListener("resize", resize);
addEventListener("pointermove", (e) => { pointer = { x: e.clientX / innerWidth, y: e.clientY / innerHeight }; });
addEventListener("pointerleave", () => { pointer = null; });
wprs.onProperties((changed) => {
  props = Object.assign(props, changed);
  fill();
  time();
  if ("photos" in changed) show();
});
wprs.onAudio(({ left, right }) => {
  const bass = (left[0] + left[1] + right[0] + right[1]) / 4;
  level = level * 0.6 + bass * 0.4;
});
wprs.onPause((paused) => {
  running = !paused;
  localStorage.setItem("lights", JSON.stringify(lights.map(({ x, y, a, r }) => ({ x, y, a, r }))));
  if (running) { last = performance.now(); requestAnimationFrame(step); }
});
resize();
fill();
time();
show();
setInterval(time, 10000);
setInterval(show, 60000);
requestAnimationFrame(step);
