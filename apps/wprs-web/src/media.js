(() => {
  if (!window.HTMLMediaElement) return;
  const largest = 268435456;
  const full = (url) => {
    try {
      return new URL(String(url), document.baseURI).href;
    } catch (e) {
      return "";
    }
  };
  const ours = (url) => /^wpak(-user)?:/i.test(full(url));
  const set = Element.prototype.setAttribute;
  const playing = HTMLMediaElement.prototype.play;
  const wanted = new WeakMap();
  const pending = new WeakMap();
  const held = new WeakMap();
  const kept = new FinalizationRegistry((url) => URL.revokeObjectURL(url));
  const fetched = (url) =>
    fetch(url).then((reply) => {
      const size = Number(reply.headers.get("content-length") || 0);
      if (!reply.ok || size > largest) throw new Error(`${url} cannot play: missing or over 256 MiB`);
      return reply.blob();
    });
  const route = (element, url) => {
    const address = full(url);
    wanted.set(element, address);
    const done = fetched(address).then((blob) => {
      if (wanted.get(element) !== address) return;
      const made = URL.createObjectURL(blob);
      const old = held.get(element);
      if (old) {
        kept.unregister(element);
        URL.revokeObjectURL(old);
      }
      held.set(element, made);
      kept.register(element, made, element);
      set.call(element, "src", made);
      const owner = element.parentElement;
      if (element instanceof HTMLSourceElement && owner instanceof HTMLMediaElement) owner.load();
    });
    pending.set(element, done);
    done.catch((e) => console.error(String(e)));
    return true;
  };
  const media = (element) => element instanceof HTMLMediaElement || element instanceof HTMLSourceElement;
  const assign = (element, url) => (ours(url) ? route(element, url) : (wanted.delete(element), false));
  for (const proto of [HTMLMediaElement.prototype, HTMLSourceElement.prototype]) {
    const real = Object.getOwnPropertyDescriptor(proto, "src");
    Object.defineProperty(proto, "src", {
      configurable: true,
      enumerable: real.enumerable,
      get() {
        return wanted.get(this) || real.get.call(this);
      },
      set(value) {
        if (!assign(this, value)) real.set.call(this, value);
      },
    });
  }
  Element.prototype.setAttribute = function (name, value) {
    if (media(this) && String(name).toLowerCase() === "src" && assign(this, value)) return undefined;
    return set.call(this, name, value);
  };
  HTMLMediaElement.prototype.play = function () {
    const waiting = pending.get(this);
    return waiting ? waiting.then(() => playing.call(this)) : playing.call(this);
  };
  const Real = window.Audio;
  window.Audio = function (url) {
    const made = new Real();
    if (url !== undefined) made.src = url;
    return made;
  };
  window.Audio.prototype = Real.prototype;
  const rescue = (event) => {
    const element = event.target;
    const url = media(element) && element.getAttribute("src");
    if (url && ours(url) && !wanted.has(element)) route(element, url);
  };
  document.addEventListener("error", rescue, true);
})();
