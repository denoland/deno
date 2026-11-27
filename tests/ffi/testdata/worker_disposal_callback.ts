// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [prefix, suffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const library = `${targetDir}/${prefix}test_ffi.${suffix}`;
// Keep the library itself alive to isolate the callback allocation lifetime.
const keepAlive = Deno.dlopen(library, {});
const timeout = setTimeout(() => {
  console.error("Worker FFI disposal did not complete");
  Deno.exit(1);
}, 30_000);

try {
  for (let i = 0; i < 40; i++) {
    const progress = new Uint32Array(new SharedArrayBuffer(6 * 4));
    progress.fill(99, 2);
    const worker = new Worker(
      new URL("./worker_disposal_callback_worker.ts", import.meta.url).href,
      { type: "module" },
    );
    const ready = new Promise<void>((resolve, reject) => {
      worker.onmessage = () => resolve();
      worker.onerror = (event) => reject(new Error(event.message));
    });
    worker.postMessage({ library, progress });
    await ready;
    while (Atomics.load(progress, 0) === 0) {
      await new Promise((resolve) => setTimeout(resolve, 1));
    }
    await worker[Symbol.asyncDispose]();
    if (Atomics.load(progress, 1) !== 1) {
      throw new Error("Disposal completed before the native call returned");
    }
    for (let lane = 2; lane < 6; lane++) {
      if (Atomics.load(progress, lane) !== 0) {
        throw new Error(`Cancelled callback returned nonzero lane ${lane}`);
      }
    }
  }
  console.log("40 callbacks stopped safely");
} finally {
  clearTimeout(timeout);
  keepAlive.close();
}
