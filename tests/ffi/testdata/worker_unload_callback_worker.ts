// Copyright 2018-2026 the Deno authors. MIT license.

onmessage = ({ data: { library, callbackFirst } }) => {
  const open = () =>
    Deno.dlopen(library, {
      store_unload_callback: { parameters: ["function"], result: "void" },
    });
  // Creation order decides resource order, so cover both.
  const dylibBefore = callbackFirst ? undefined : open();
  // Same-thread callback: the library calls it from its unload destructor,
  // which runs on this worker's thread during teardown.
  const callback = new Deno.UnsafeCallback(
    { parameters: [], result: "void" },
    () => {},
  );
  const dylib = dylibBefore ?? open();
  dylib.symbols.store_unload_callback(callback.pointer);
  postMessage("stored");
  while (true) { /* interrupted by the parent */ }
};
