// Copyright 2018-2026 the Deno authors. MIT license.

onmessage = () => {
  const definition = { parameters: [], result: "i32" } as const;
  // Converting the returned object to i32 runs its valueOf in JavaScript,
  // after the callback itself has returned to native code.
  const callback = new Deno.UnsafeCallback(definition, () =>
    ({
      valueOf() {
        postMessage("coercing");
        while (true) { /* interrupted by the parent */ }
      },
    }) as unknown as number);
  new Deno.UnsafeFnPointer(callback.pointer, definition).call();
};
