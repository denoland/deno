// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

// Stopping a worker while it converts a callback's return value must return
// a zero value to native code rather than unwind across the C boundary.
const worker = new Worker(
  new URL("./worker_callback_return_coercion_worker.ts", import.meta.url).href,
  { type: "module" },
);
const coercing = new Promise<void>((resolve) => {
  worker.onmessage = () => resolve();
});
worker.postMessage(null);
await coercing;
await worker[Symbol.asyncDispose]();
console.log("interrupted return conversion stopped safely");
