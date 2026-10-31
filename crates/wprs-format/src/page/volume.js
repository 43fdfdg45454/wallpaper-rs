(() => {
  const api = (() => {
    try {
      return window.wprs || window.top.wprs;
    } catch (e) {
      return undefined;
    }
  })();
  if (!api || !window.HTMLMediaElement) return;
  const level = () => Math.min(1, Math.max(0, Number(api.volume) || 0));
  const media = HTMLMediaElement.prototype;
  const real = Object.getOwnPropertyDescriptor(media, "volume");
  const wanted = new WeakMap();
  const known = new WeakSet();
  const elements = new Set();
  const apply = (element) => {
    if (!known.has(element)) {
      known.add(element);
      elements.add(new WeakRef(element));
    }
    if (!wanted.has(element)) wanted.set(element, real.get.call(element));
    real.set.call(element, wanted.get(element) * level());
  };
  Object.defineProperty(media, "volume", {
    configurable: true,
    enumerable: real.enumerable,
    get() {
      return wanted.has(this) ? wanted.get(this) : real.get.call(this);
    },
    set(value) {
      real.set.call(this, value);
      wanted.set(this, real.get.call(this));
      apply(this);
    },
  });
  const play = media.play;
  media.play = function () {
    apply(this);
    return play.call(this);
  };
  for (const name of ["loadstart", "play"]) {
    document.addEventListener(name, (event) => event.target instanceof HTMLMediaElement && apply(event.target), true);
  }
  const gains = new Set();
  const base = window.BaseAudioContext && Object.getOwnPropertyDescriptor(BaseAudioContext.prototype, "destination");
  const nodes = new WeakMap();
  if (window.AudioContext && base) {
    Object.defineProperty(AudioContext.prototype, "destination", {
      configurable: true,
      get() {
        if (!nodes.has(this)) {
          const out = base.get.call(this);
          const node = this.createGain();
          node.gain.value = level();
          node.connect(out);
          Object.defineProperty(node, "maxChannelCount", { get: () => out.maxChannelCount });
          nodes.set(this, node);
          gains.add(new WeakRef(node));
        }
        return nodes.get(this);
      },
    });
  }
  const each = (refs, f) => refs.forEach((ref) => (ref.deref() ? f(ref.deref()) : refs.delete(ref)));
  api.onSettings(() => {
    each(elements, apply);
    each(gains, (node) => (node.gain.value = level()));
  });
})();
