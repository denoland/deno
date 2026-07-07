// Copyright 2018-2026 the Deno authors. MIT license.

const worker = new Worker(new URL("./worker.js", import.meta.url), {
  type: "module",
});

const message = await new Promise((resolve) => {
  worker.onmessage = (event) => resolve(event.data);
});

if (message !== "looping") {
  throw new Error(`Unexpected worker message: ${message}`);
}

console.log("terminating");
worker.terminate();
