let _location = undefined;

console.log(globalThis.location);

Object.defineProperty(globalThis, "location", {
  get() {
    return _location;
  },
  set(v) {
    _location = v;
  },
  configurable: true,
});

console.log(globalThis.location);

globalThis.location = "https://deno.com";

console.log(_location);
console.log(location);

delete globalThis["location"];

console.log(globalThis.location);
