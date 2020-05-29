Object.defineProperty(globalThis, Symbol.toStringTag, {
  value: "global",
  writable: false,
  enumerable: false,
  configurable: true,
});

// @ts-expect-error
globalThis["global"] = globalThis;
