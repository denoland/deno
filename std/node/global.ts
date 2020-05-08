Object.defineProperty(globalThis, Symbol.toStringTag, {
  value: "global",
  writable: false,
  enumerable: false,
  configurable: true,
});

// @ts-ignore
globalThis["global"] = globalThis;
