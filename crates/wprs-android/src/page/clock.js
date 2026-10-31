(g, own) => {
  const Real = g.Date;
  const p = Real.prototype;
  const time = p.getTime;
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  const two = (n) => String(n).padStart(2, "0");
  const local = ["FullYear", "Month", "Date", "Hours", "Minutes", "Seconds", "Milliseconds"].map((part) => p[`get${part}`]);
  const wall = (t) => {
    const [y, m, day, ...rest] = local.map((get) => get.call(new Real(t)));
    const d = new Real(0);
    d.setUTCFullYear(y, m, day);
    return d.setUTCHours(...rest);
  };
  const iso = /^[+-]?\d{4,6}-\d\d-\d\dT\d\d:\d\d(:\d\d(\.\d+)?)?$/;
  const day = /^[+-]?\d{4,6}(-\d\d){0,2}$/;
  const zoned = /\d(Z|[+-]\d\d:?\d\d)\b|\d\s+[+-]\d{4}\b|\b(UTC?|GMT|Z|[ECMP][SD]T)\b/i;
  const parse = (given) => {
    const s = String(given).trim();
    if (iso.test(s)) return Real.parse(`${s}Z`);
    if (day.test(s) || zoned.test(s)) return Real.parse(s);
    const t = Real.parse(`${s} GMT`);
    return isNaN(t) ? wall(Real.parse(s)) : t;
  };
  const dated = (v) => {
    try {
      return time.call(v), true;
    } catch (e) {
      return false;
    }
  };
  const primitive = (v) => {
    if (Object(v) !== v || dated(v)) return v;
    const made = v[Symbol.toPrimitive];
    return typeof made === "function" ? made.call(v, "default") : p[Symbol.toPrimitive].call(v, "number");
  };
  for (const part of ["FullYear", "Month", "Date", "Day", "Hours", "Minutes", "Seconds", "Milliseconds"]) {
    p[`get${part}`] = p[`getUTC${part}`];
    if (part !== "Day") p[`set${part}`] = p[`setUTC${part}`];
  }
  p.getTimezoneOffset = () => 0;
  p.getYear = function () {
    return this.getUTCFullYear() - 1900;
  };
  p.setYear = function (year) {
    const n = Number(year);
    const i = Math.trunc(n);
    if (isNaN(this)) this.setTime(0);
    return this.setUTCFullYear(i >= 0 && i <= 99 ? 1900 + i : n);
  };
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
    const one = args.length === 1 ? primitive(args[0]) : undefined;
    const given = args.length > 1 ? [Real.UTC(...args)] : args.length ? [typeof one === "string" ? parse(one) : one] : [];
    return Reflect.construct(Real, given, new.target === Fake ? Real : new.target);
  };
  Fake.prototype = p;
  own(p, Fake);
  Object.assign(Fake, { UTC: Real.UTC, now: Real.now, parse });
  g.Date = Fake;
  const modified = g.Document && Object.getOwnPropertyDescriptor(g.Document.prototype, "lastModified");
  if (modified) {
    Object.defineProperty(g.Document.prototype, "lastModified", {
      configurable: true,
      get() {
        const d = new Real(Real.parse(modified.get.call(this)));
        const at = [d.getUTCHours(), d.getUTCMinutes(), d.getUTCSeconds()].map(two).join(":");
        return `${two(d.getUTCMonth() + 1)}/${two(d.getUTCDate())}/${d.getUTCFullYear()} ${at}`;
      },
    });
  }
  const now = g.Temporal && g.Temporal.Now;
  for (const name of now ? Object.getOwnPropertyNames(now) : []) {
    const real = now[name];
    if (typeof real !== "function") continue;
    now[name] = name === "timeZoneId" ? () => "UTC" : (at, ...rest) => real.call(now, at === undefined ? "UTC" : at, ...rest);
  }
}
