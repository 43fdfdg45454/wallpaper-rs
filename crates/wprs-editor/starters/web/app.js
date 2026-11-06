const time = document.getElementById("time");
let timer = 0;

function tick() {
  time.textContent = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  timer = setTimeout(tick, 60000 - (Date.now() % 60000));
}

wprs.onProperties((changed) => {
  if (changed.color) document.body.style.color = changed.color.hex;
});

wprs.onPause((paused) => {
  clearTimeout(timer);
  if (!paused) tick();
});

tick();
