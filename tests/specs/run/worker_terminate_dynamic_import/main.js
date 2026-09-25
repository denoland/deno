// Copyright 2018-2026 the Deno authors. MIT license.

const worker = new Worker(new URL("./worker.js", import.meta.url), {
  type: "module",
});

await new Promise((resolve, reject) => {
  worker.onmessage = resolve;
  worker.onerror = reject;
});

worker.terminate();
console.log("terminated");
