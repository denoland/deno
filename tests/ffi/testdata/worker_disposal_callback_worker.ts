// Copyright 2018-2026 the Deno authors. MIT license.

onmessage = ({ data: { library, progress } }) => {
  const dylib = Deno.dlopen(library, {
    callback_during_worker_disposal: {
      parameters: ["function", "buffer"],
      result: "void",
      nonblocking: true,
    },
  });
  const callback = Deno.UnsafeCallback.threadSafe(
    { parameters: [], result: { struct: ["u64", "u64", "u64", "u64"] } },
    () => new Uint8Array(32).fill(1),
  );
  void dylib.symbols.callback_during_worker_disposal(
    callback.pointer,
    progress,
  );
  postMessage("native call submitted");
  // The event loop cannot dispatch the native callback before termination.
  while (true) { /* interrupted by the parent */ }
};
