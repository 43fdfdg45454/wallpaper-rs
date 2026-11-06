(() => {
  const init = __WPRS_INIT__;
  const post = __WPRS_POST__;
  const handlers = { properties: [], settings: [], pause: [], audio: [], files: new Map(), action: [], media: [] };
  let heard = { enabled: false };
  let picture = null;
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
  let lit = 0;
  let lights = init.lights === true;
  const shrink = (bytes, w, h) => {
    const scale = Math.min(64 / w, 16 / h, 1);
    const sw = Math.max(1, Math.round(w * scale)), sh = Math.max(1, Math.round(h * scale));
    const data = [];
    for (let y = 0; y < sh; y++) {
      for (let x = 0; x < sw; x++) {
        const x0 = Math.floor(x * w / sw), x1 = Math.max(x0 + 1, Math.floor((x + 1) * w / sw));
        const y0 = Math.floor(y * h / sh), y1 = Math.max(y0 + 1, Math.floor((y + 1) * h / sh));
        const sx = Math.max(1, Math.floor((x1 - x0) / 4)), sy = Math.max(1, Math.floor((y1 - y0) / 4));
        const sum = [0, 0, 0];
        let n = 0;
        for (let yy = y0; yy < y1; yy += sy) {
          for (let xx = x0; xx < x1; xx += sx) {
            const i = (yy * w + xx) * 4;
            sum[0] += bytes[i];
            sum[1] += bytes[i + 1];
            sum[2] += bytes[i + 2];
            n += 1;
          }
        }
        data.push(...sum.map((s) => Math.round(s / n)));
      }
    }
    return { width: sw, height: sh, data };
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
    onMedia(cb) { handlers.media.push(cb); if (handlers.media.length === 1) post({ media: true }); else call([cb], Object.assign({}, heard)); },
    openShortcut(prop) {
      if (navigator.userActivation && !navigator.userActivation.isActive) return false;
      post({ open: String(prop) });
      return true;
    },
    rgb: Object.freeze({
      setImage(bytes, w, h) {
        const now = Date.now();
        const width = Math.floor(Number(w)), height = Math.floor(Number(h));
        const fine = width > 0 && height > 0 && width <= 4096 && height <= 4096 && bytes && bytes.length >= width * height * 4;
        if (!fine || !lights || now - lit < 33) return false;
        lit = now;
        post({ rgb: shrink(bytes, width, height) });
        return true;
      },
    }),
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
    if (message.media) {
      const m = message.media;
      if (typeof m.cover === "string") {
        if (picture) URL.revokeObjectURL(picture);
        const bytes = m.cover ? Uint8Array.from(atob(m.cover), (c) => c.charCodeAt(0)) : null;
        picture = bytes ? URL.createObjectURL(new Blob([bytes], { type: "image/png" })) : null;
      }
      const colors = m.colors ? Object.fromEntries(Object.entries(m.colors).map(([k, v]) => [k, color(v)])) : null;
      const { state, title, artist, album, position, duration, rate } = m;
      heard = m.enabled ? { enabled: true, state, title, artist, album, cover: picture, colors, position, duration, rate } : { enabled: false };
      call(handlers.media, Object.assign({}, heard));
    }
    if (typeof message.lights === "boolean") lights = message.lights;
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
