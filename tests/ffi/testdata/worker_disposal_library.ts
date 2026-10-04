// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

// Only the worker opens the library, so unloading it while the worker's
// managed native call is still running would return into unmapped code.
const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [prefix, suffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const library = `${targetDir}/${prefix}test_ffi.${suffix}`;

for (let i = 0; i < 10; i++) {
  const worker = new Worker(
    new URL("./worker_disposal_library_worker.ts", import.meta.url).href,
    { type: "module" },
  );
  const submitted = new Promise<void>((resolve) => {
    worker.onmessage = () => resolve();
  });
  worker.postMessage({ library });
  await submitted;
  await worker[Symbol.asyncDispose]();
}
console.log("10 native calls returned into a loaded library");
