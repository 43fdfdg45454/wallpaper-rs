const Base = Date;
const off = 3 * 3600000;
const zone = "America/Argentina/Buenos_Aires";
const supported = ["en-US", "es-AR", "ja-JP"];
const canonical = (l) => {
  if (l === undefined) return [];
  if (typeof l === "string") return [l];
  if (l === null) throw new TypeError("locales");
  const o = Object(l);
  return Array.from({ length: o.length >>> 0 }, (_, i) => i).filter((i) => i in o).map((i) => String(o[i]));
};
function Format(l, o) {
  if (!new.target) return new Format(l, o);
  const at = o === undefined ? undefined : Object(o).timeZone;
  const locale = canonical(l).find((t) => supported.includes(t)) || "es-AR";
  this.options = { locale, timeZone: at === undefined ? zone : String(at) };
}
Format.prototype.resolvedOptions = function () {
  return { ...this.options };
};
const shown = (l, o) => Object.values(new Format(l, o).resolvedOptions()).join(" ");
globalThis.Intl = {
  getCanonicalLocales: canonical,
  DateTimeFormat: Format,
  NumberFormat: function (l, o) {
    return new Format(l, o);
  },
};
const p = Base.prototype;
for (const part of ["FullYear", "Month", "Date", "Day", "Hours", "Minutes", "Seconds", "Milliseconds"]) {
  p[`get${part}`] = function () {
    return new Base(this.getTime() - off)[`getUTC${part}`]();
  };
}
p.getTimezoneOffset = () => off / 60000;
p.getYear = function () {
  return new Base(this.getTime() - off).getUTCFullYear() - 1900;
};
p.toLocaleString = shown;
Number.prototype.toLocaleString = (l, o) => shown(l, o).split(" ")[0];
const legacy = /^(\d\d)\/(\d\d)\/(\d{4}) (\d\d):(\d\d):(\d\d)( GMT)?$/;
const parse = (given) => {
  const s = String(given);
  const m = legacy.exec(s);
  if (m) return Base.UTC(m[3], m[1] - 1, m[2], m[4], m[5], m[6]) + (m[7] ? 0 : off);
  return /T[\d:.]+$/.test(s) ? Base.parse(`${s}Z`) + off : Base.parse(s);
};
function Zoned(...a) {
  if (!new.target) return String(new Zoned());
  const [v] = a;
  const text = typeof v === "string" || v instanceof String;
  const t = a.length > 1 ? Base.UTC(...a) + off : text ? parse(v) : a.length ? v : Base.now();
  return Reflect.construct(Base, [t], new.target);
}
Zoned.prototype = p;
Object.defineProperty(p, "constructor", { value: Zoned, writable: true, configurable: true });
Object.assign(Zoned, { UTC: Base.UTC, now: Base.now, parse });
globalThis.Date = Zoned;
globalThis.Document = class {};
Object.defineProperty(Document.prototype, "lastModified", { configurable: true, get: () => "01/01/2024 09:00:00" });
globalThis.Temporal = { Instant: class {} };
Temporal.Instant.prototype.toLocaleString = shown;
