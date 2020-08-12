Object.defineProperty(globalThis, Symbol.toStringTag, {
  value: "global",
  writable: false,
  enumerable: false,
  configurable: true,
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
(globalThis as any)["global"] = globalThis;

export {};
