(allowed, post) => {
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
}
