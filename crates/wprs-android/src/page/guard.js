(zone, language, mask, origin, clocks) => {
  const mark = Symbol.for("wprs.guard");
  const tag = "en-US";
  const peers = ["RTCPeerConnection", "webkitRTCPeerConnection", "RTCDataChannel", "RTCIceCandidate", "RTCSessionDescription"];
  const formats = ["DateTimeFormat", "NumberFormat", "Collator", "PluralRules", "RelativeTimeFormat", "ListFormat", "DisplayNames", "Segmenter", "DurationFormat"];
  const listed = language && Intl.getCanonicalLocales;
  const pick = (locales) => (language ? [...listed(locales), tag] : locales);
  const timed = (given) => {
    if (!zone || given === null) return given;
    const at = given === undefined ? undefined : given.timeZone;
    const value = at === undefined ? "UTC" : at;
    return Object.create(given === undefined ? null : Object(given), { timeZone: { value, enumerable: true } });
  };
  const own = (proto, made) => Object.defineProperty(proto, "constructor", { value: made, writable: true, configurable: true });
  const replace = (proto, name, wrap) => {
    if (proto && typeof proto[name] === "function") proto[name] = wrap(proto[name]);
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
    const temporal = ["Instant", "ZonedDateTime", "PlainDate", "PlainTime", "PlainDateTime", "PlainYearMonth", "PlainMonthDay", "Duration"];
    for (const kind of g.Temporal ? temporal : []) {
      replace(g.Temporal[kind] && g.Temporal[kind].prototype, "toLocaleString", (real) => function (locales, given) {
        return real.call(this, pick(locales), kind === "Instant" ? timed(given) : given);
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
    if (zone) clocks(g, own);
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
