Object.defineProperty(globalThis, "_", {
  configurable: true,
  get: () => Deno[Deno.internal].lastEvalResult,
  set: (value) => {
    Object.defineProperty(globalThis, "_", {
      value: value,
      writable: true,
      enumerable: true,
      configurable: true,
    });
    console.log("Last evaluation result is no longer saved to _.");
  },
});

Object.defineProperty(globalThis, "_error", {
  configurable: true,
  get: () => Deno[Deno.internal].lastThrownError,
  set: (value) => {
    Object.defineProperty(globalThis, "_error", {
      value: value,
      writable: true,
      enumerable: true,
      configurable: true,
    });

    console.log("Last thrown error is no longer saved to _error.");
  },
});
