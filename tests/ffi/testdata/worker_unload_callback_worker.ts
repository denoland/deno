// Copyright 2018-2026 the Deno authors. MIT license.

onmessage = ({ data: { library } }) => {
  const dylib = Deno.dlopen(library, {
    store_unload_callback: { parameters: ["function"], result: "void" },
  });
  // Same-thread callback: the library calls it from its unload destructor,
  // which runs on this worker's thread during teardown.
  const callback = new Deno.UnsafeCallback(
    { parameters: [], result: "void" },
    () => {},
  );
  dylib.symbols.store_unload_callback(callback.pointer);
  postMessage("stored");
  while (true) { /* interrupted by the parent */ }
};
