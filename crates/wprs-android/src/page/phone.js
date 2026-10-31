(() => {
  const native = window.requestAnimationFrame.bind(window);
  let fps = wprs.fps || 60;
  let last = 0;
  let pending = false;
  let next = 1;
  let queue = new Map();
  const run = (now) => {
    const wait = 1000 / fps - 2 - (now - last);
    if (wait > 0) {
      setTimeout(() => native(run), wait);
      return;
    }
    pending = false;
    last = now;
    const due = queue;
    queue = new Map();
    due.forEach((cb) => {
      try {
        cb(now);
      } catch (e) {
        setTimeout(() => { throw e; });
      }
    });
  };
  window.requestAnimationFrame = (cb) => {
    queue.set(next, cb);
    if (!pending) {
      pending = true;
      native(run);
    }
    return next++;
  };
  window.cancelAnimationFrame = (id) => queue.delete(id);
  const contexts = [];
  const Real = window.AudioContext;
  if (Real) {
    window.AudioContext = function (...args) {
      const made = new Real(...args);
      contexts.push(made);
      if (wprs.volume <= 0) made.suspend();
      return made;
    };
    window.AudioContext.prototype = Real.prototype;
  }
  const silenced = new WeakSet();
  const quiet = (element) => {
    if (wprs.volume <= 0 && !element.muted) {
      element.muted = true;
      silenced.add(element);
    }
  };
  document.addEventListener("play", (event) => quiet(event.target), true);
  wprs.onSettings((settings) => {
    fps = settings.fps || 60;
    const muted = settings.volume <= 0;
    contexts.forEach((c) => (muted ? c.suspend() : c.resume()));
    document.querySelectorAll("audio, video").forEach((element) => {
      if (muted) quiet(element);
      else if (silenced.has(element)) {
        element.muted = false;
        silenced.delete(element);
      }
    });
  });
})();
