// Copyright 2018-2026 the Deno authors. MIT license.

onmessage = ({ data: { library } }) => {
  const dylib = Deno.dlopen(library, {
    sleep_blocking: { parameters: ["u64"], result: "void", nonblocking: true },
  });
  void dylib.symbols.sleep_blocking(200n);
  postMessage("native call submitted");
  while (true) { /* interrupted by the parent */ }
};
