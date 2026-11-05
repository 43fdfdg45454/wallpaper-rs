(allowed, post) => {
  const mark = Symbol.for("wprs.kept");
  if (window[mark]) return;
  window[mark] = true;
  let pending = false;
  const later = typeof setTimeout === "function" ? setTimeout : (run) => run();
  const wrote = () => {
    if (pending) return;
    pending = true;
    later(() => {
      pending = false;
      post({ wrote: true });
    }, 2000);
  };
  const watch = (made, names) => {
    if (typeof made !== "function") return;
    for (const name of names) {
      const real = made.prototype[name];
      if (typeof real !== "function") continue;
      made.prototype[name] = function (...args) {
        wrote();
        return real.apply(this, args);
      };
    }
  };
  watch(window.IDBObjectStore, ["put", "add"]);
  watch(window.IDBCursor, ["update"]);
  watch(window.Cache, ["put", "add", "addAll"]);
  let store = null;
  try {
    store = window.localStorage;
  } catch (e) {}
  if (!store) return;
  const weight = (key, value) => (value === null ? 0 : (String(key).length + String(value).length) * 2);
  const measure = () => {
    let total = 0;
    for (let i = 0; i < store.length; i += 1) total += weight(store.key(i), store.getItem(store.key(i)));
    return total;
  };
  let total = measure();
  post({ kept: total });
  const proto = Storage.prototype;
  const set = proto.setItem;
  const remove = proto.removeItem;
  const clear = proto.clear;
  proto.setItem = function (key, value) {
    if (this !== store) return set.call(this, key, value);
    const grown = weight(key, value) - weight(key, store.getItem(String(key)));
    if (total + grown > allowed) total = measure();
    if (grown > 0 && total + grown > allowed) {
      throw new DOMException("the page keeps more data than it declared", "QuotaExceededError");
    }
    set.call(this, key, value);
    total += grown;
  };
  proto.removeItem = function (key) {
    if (this === store) total -= weight(key, store.getItem(String(key)));
    return remove.call(this, key);
  };
  proto.clear = function () {
    if (this === store) total = 0;
    return clear.call(this);
  };
  const own = new Set(Object.getOwnPropertyNames(proto));
  const named = (key) => typeof key === "string" && !own.has(key);
  const shown = new Proxy(store, {
    get: (target, key) => {
      const value = Reflect.get(target, key, target);
      return typeof value === "function" ? value.bind(target) : value;
    },
    set: (target, key, value) => {
      if (!named(key)) return Reflect.set(target, key, value, target);
      target.setItem(key, value);
      return true;
    },
    deleteProperty: (target, key) => {
      if (named(key)) target.removeItem(key);
      return true;
    },
  });
  try {
    Object.defineProperty(window, "localStorage", { get: () => shown, configurable: true, enumerable: true });
  } catch (e) {}
}
