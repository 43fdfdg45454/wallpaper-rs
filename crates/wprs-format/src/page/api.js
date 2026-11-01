(() => {
  const init = __WPRS_INIT__;
  const post = __WPRS_POST__;
  const handlers = { properties: [], settings: [], pause: [], audio: [], files: new Map(), action: [] };
  const waiting = new Map();
  let asked = 0;
  let paused = document.visibilityState === "hidden";
  const color = (value) => {
    const hex = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(String(value));
    if (!hex) return value;
    const n = parseInt(hex[1], 16);
    const a = hex[2] ? parseInt(hex[2], 16) / 255 : 1;
    return { hex: value, r: (n >> 16) / 255, g: ((n >> 8) & 255) / 255, b: (n & 255) / 255, a };
  };
  const typed = (values) => {
    const out = {};
    for (const [id, value] of Object.entries(values)) {
      const kind = init.kinds[id];
      out[id] = kind === "color" ? color(value) : kind === "combo" ? { value, label: init.labels[id]?.[String(value)] ?? String(value) } : value;
    }
    return out;
  };
  const properties = typed(init.properties);
  const settings = Object.assign({}, init.settings);
  const call = (list, value) => list.forEach((cb) => { try { cb(value); } catch (e) { post({ error: String(e && e.stack || e) }); } });
  const ask = (kind, prop) => new Promise((resolve) => {
    asked += 1;
    waiting.set(asked, resolve);
    post({ [kind]: prop, id: asked });
  });
  const api = {
    properties,
    get fps() { return settings.fps; },
    get volume() { return settings.volume; },
    get locale() { return settings.locale; },
    get quality() { return settings.quality; },
    get paused() { return paused; },
    onProperties(cb) { handlers.properties.push(cb); call([cb], Object.assign({}, properties)); },
    onSettings(cb) { handlers.settings.push(cb); call([cb], Object.assign({}, settings)); },
    onPause(cb) { handlers.pause.push(cb); },
    onAudio(cb) { handlers.audio.push(cb); if (handlers.audio.length === 1) post({ audio: true }); },
    onAction(cb) { handlers.action.push(cb); },
    onMedia(cb) { call([cb], { enabled: false }); },
    openShortcut(prop) {
      if (navigator.userActivation && !navigator.userActivation.isActive) return false;
      post({ open: String(prop) });
      return true;
    },
    rgb: Object.freeze({ setImage() { return false; } }),
    files(prop) { return ask("files", prop); },
    randomFile(prop) { return ask("random", prop); },
    onFiles(prop, cb) {
      const list = handlers.files.get(prop) || [];
      list.push(cb);
      handlers.files.set(prop, list);
    },
  };
  const receive = (message) => {
    if (message.properties) {
      const changed = typed(message.properties);
      Object.assign(properties, changed);
      call(handlers.properties, changed);
    }
    if (message.settings) {
      Object.assign(settings, message.settings);
      call(handlers.settings, Object.assign({}, settings));
    }
    if (message.audio) {
      const [left, right] = message.audio.map((bands) => Float32Array.from(bands));
      call(handlers.audio, { left, right });
    }
    if (message.files) call(handlers.files.get(message.files.prop) || [], message.files);
    if (message.action) call(handlers.action, message.action);
    if (message.reply && waiting.has(message.reply.id)) {
      waiting.get(message.reply.id)(message.reply.value);
      waiting.delete(message.reply.id);
    }
  };
  document.addEventListener("visibilitychange", () => {
    const now = document.visibilityState === "hidden";
    if (now !== paused) {
      paused = now;
      call(handlers.pause, paused);
    }
  });
  window.addEventListener("error", (event) => post({ error: String(event.message) }));
  for (const level of ["log", "info", "warn", "error", "debug"]) {
    const original = console[level];
    console[level] = (...args) => {
      post({ error: `${level}: ${args.map(String).join(" ")}` });
      original.apply(console, args);
    };
  }
  Object.defineProperty(window, "wprs", { value: Object.freeze(api), enumerable: true });
  Object.defineProperty(window, "__wprs", { value: receive });
})();
