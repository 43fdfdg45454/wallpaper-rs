(zone, language, mask, origin) => {
  const mark = Symbol.for("wprs.guard");
  const tag = "en-US";
  const peers = ["RTCPeerConnection", "webkitRTCPeerConnection", "RTCDataChannel", "RTCIceCandidate", "RTCSessionDescription"];
  const formats = ["DateTimeFormat", "NumberFormat", "Collator", "PluralRules", "RelativeTimeFormat", "ListFormat", "DisplayNames", "Segmenter", "DurationFormat"];
  const pick = (locales) => (language && locales === undefined ? tag : locales);
  const timed = (given) => (zone ? Object.assign({ timeZone: "UTC" }, given) : given);
  const own = (proto, made) => Object.defineProperty(proto, "constructor", { value: made, writable: true, configurable: true });
  const replace = (proto, name, wrap) => {
    if (proto && typeof proto[name] === "function") proto[name] = wrap(proto[name]);
  };
  const clocks = (g) => {
    const Real = g.Date;
    const p = Real.prototype;
    const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const two = (n) => String(n).padStart(2, "0");
    for (const part of ["FullYear", "Month", "Date", "Day", "Hours", "Minutes", "Seconds", "Milliseconds"]) {
      p[`get${part}`] = p[`getUTC${part}`];
      if (part !== "Day") p[`set${part}`] = p[`setUTC${part}`];
    }
    p.getTimezoneOffset = () => 0;
    p.toDateString = function () {
      return `${days[this.getUTCDay()]} ${months[this.getUTCMonth()]} ${two(this.getUTCDate())} ${this.getUTCFullYear()}`;
    };
    p.toTimeString = function () {
      return `${two(this.getUTCHours())}:${two(this.getUTCMinutes())}:${two(this.getUTCSeconds())} GMT+0000 (Coordinated Universal Time)`;
    };
    p.toString = function () {
      return isNaN(this) ? "Invalid Date" : `${this.toDateString()} ${this.toTimeString()}`;
    };
    const Fake = function (...args) {
      if (!new.target) return new Real().toString();
      const local = args.length > 1 ? [Real.UTC(...args)] : args;
      return Reflect.construct(Real, local, new.target === Fake ? Real : new.target);
    };
    Fake.prototype = p;
    own(p, Fake);
    Object.assign(Fake, { UTC: Real.UTC, now: Real.now, parse: Real.parse });
    g.Date = Fake;
    const now = g.Temporal && g.Temporal.Now;
    for (const name of now ? Object.getOwnPropertyNames(now) : []) {
      const real = now[name];
      if (typeof real !== "function") continue;
      now[name] = name === "timeZoneId" ? () => "UTC" : (at, ...rest) => real.call(now, at === undefined ? "UTC" : at, ...rest);
    }
  };
  const hide = (g) => {
    for (const name of formats) {
      const Real = g.Intl[name];
      if (!Real) continue;
      const dated = name === "DateTimeFormat";
      const Made = function (locales, given) {
        return new Real(pick(locales), dated ? timed(given) : given);
      };
      Made.prototype = Real.prototype;
      Made.supportedLocalesOf = Real.supportedLocalesOf;
      own(Real.prototype, Made);
      g.Intl[name] = Made;
    }
    for (const name of ["toLocaleString", "toLocaleDateString", "toLocaleTimeString"]) {
      replace(g.Date.prototype, name, (real) => function (locales, given) {
        return real.call(this, pick(locales), timed(given));
      });
    }
    const located = (real) => function (locales, given) {
      return real.call(this, pick(locales), given);
    };
    [g.Number, g.BigInt].forEach((kind) => replace(kind && kind.prototype, "toLocaleString", located));
    ["toLocaleUpperCase", "toLocaleLowerCase"].forEach((name) => replace(g.String.prototype, name, located));
    replace(g.String.prototype, "localeCompare", (real) => function (that, locales, given) {
      return real.call(this, that, pick(locales), given);
    });
    const nav = g.Navigator || g.WorkerNavigator;
    if (language && nav) {
      Object.defineProperty(nav.prototype, "language", { get: () => tag, configurable: true });
      Object.defineProperty(nav.prototype, "languages", { get: () => Object.freeze([tag]), configurable: true });
    }
    if (zone) clocks(g);
  };
  const workers = (g) => {
    for (const name of ["Worker", "SharedWorker"]) {
      const Real = g[name];
      if (!Real) continue;
      const Made = function (url, options) {
        if (!new.target) return Real(url, options);
        const base = g.document ? g.document.baseURI : g.location.href;
        const kind = options && options.type === "module" ? "module" : "classic";
        const boot = `${origin}/:wprs/worker.js?${kind}&${encodeURIComponent(new URL(String(url), base).href)}`;
        return Reflect.construct(Real, [boot, options], new.target === Made ? Real : new.target);
      };
      Made.prototype = Real.prototype;
      own(Real.prototype, Made);
      g[name] = Made;
    }
    const ready = new WeakMap();
    replace(g.Worklet && g.Worklet.prototype, "addModule", (add) => function (url, options) {
      if (!ready.has(this)) ready.set(this, add.call(this, `${origin}/:wprs/guard.js`));
      return ready.get(this).then(() => add.call(this, url, options));
    });
  };
  const inserts = [
    ["Node", ["appendChild", "insertBefore", "replaceChild"]],
    ["Element", ["append", "prepend", "before", "after", "replaceWith", "replaceChildren", "insertAdjacentElement", "insertAdjacentHTML", "setHTMLUnsafe"]],
    ["CharacterData", ["before", "after", "replaceWith"]],
    ["Document", ["write", "writeln", "append", "prepend", "replaceChildren"]],
    ["Range", ["insertNode", "surroundContents"]],
  ];
  const frames = (g) => {
    if (!g.document) return;
    const sweep = () => {
      for (let i = 0; i < g.length; i += 1) {
        try {
          guard(g[i]);
        } catch (e) {}
      }
    };
    const after = (real) => function (...args) {
      try {
        return real.apply(this, args);
      } finally {
        sweep();
      }
    };
    for (const [kind, names] of inserts) names.forEach((name) => replace(g[kind] && g[kind].prototype, name, after));
    for (const kind of [g.Element, g.ShadowRoot].filter(Boolean)) {
      for (const name of ["innerHTML", "outerHTML"]) {
        const real = Object.getOwnPropertyDescriptor(kind.prototype, name);
        if (!real || !real.set) continue;
        Object.defineProperty(kind.prototype, name, { configurable: true, get: real.get, set: after(real.set) });
      }
    }
    new g.MutationObserver(sweep).observe(g.document, { childList: true, subtree: true });
    g.document.addEventListener("load", sweep, true);
    for (const element of [g.HTMLIFrameElement, g.HTMLFrameElement].filter(Boolean)) {
      for (const name of ["contentWindow", "contentDocument"]) {
        const real = Object.getOwnPropertyDescriptor(element.prototype, name);
        Object.defineProperty(element.prototype, name, {
          configurable: true,
          get() {
            const value = real.get.call(this);
            try {
              if (value) guard(value.defaultView || value);
            } catch (e) {}
            return value;
          },
        });
      }
    }
  };
  const guard = (g) => {
    if (!g || g[mark]) return;
    Object.defineProperty(g, mark, { value: true });
    for (const name of peers) delete g[name];
    if (g.Navigator) Object.defineProperty(g.Navigator.prototype, "serviceWorker", { get: () => undefined, configurable: true });
    if (zone || language) hide(g);
    if (mask) mask(g);
    workers(g);
    frames(g);
  };
  guard(globalThis);
}
