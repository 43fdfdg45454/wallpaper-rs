(zone, language) => {
  const tag = "en-US";
  const pick = (locales) => (language && locales === undefined ? tag : locales);
  const options = (given) => (zone ? Object.assign({ timeZone: "UTC" }, given) : given);
  for (const name of ["DateTimeFormat", "NumberFormat", "Collator", "PluralRules", "RelativeTimeFormat", "ListFormat"]) {
    const Real = Intl[name];
    if (!Real) continue;
    const timed = name === "DateTimeFormat";
    const Made = function (locales, given) {
      return new Real(pick(locales), timed ? options(given) : given);
    };
    Made.prototype = Real.prototype;
    Made.supportedLocalesOf = Real.supportedLocalesOf;
    Intl[name] = Made;
  }
  const p = Date.prototype;
  for (const name of ["toLocaleString", "toLocaleDateString", "toLocaleTimeString"]) {
    const real = p[name];
    p[name] = function (locales, given) {
      return real.call(this, pick(locales), options(given));
    };
  }
  const number = Number.prototype.toLocaleString;
  Number.prototype.toLocaleString = function (locales, given) {
    return number.call(this, pick(locales), given);
  };
  if (language) {
    Object.defineProperty(Navigator.prototype, "language", { get: () => tag });
    Object.defineProperty(Navigator.prototype, "languages", { get: () => Object.freeze([tag]) });
  }
  if (!zone) return;
  const Real = Date;
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
    const time = `${two(this.getUTCHours())}:${two(this.getUTCMinutes())}:${two(this.getUTCSeconds())}`;
    return `${time} GMT+0000 (Coordinated Universal Time)`;
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
  Object.assign(Fake, { UTC: Real.UTC, now: Real.now, parse: Real.parse });
  window.Date = Fake;
}
