// Copyright 2018-2026 the Deno authors. MIT license.

const worker = new Worker(new URL("./worker.js", import.meta.url), {
  type: "module",
});

const iterations = await new Promise((resolve) => {
  worker.onmessage = (event) => resolve(event.data);
});

if (!(iterations instanceof Int32Array)) {
  throw new Error(`Unexpected worker message: ${iterations}`);
}
while (Atomics.load(iterations, 0) === 0) {
  Atomics.wait(iterations, 0, 0);
}

console.log("terminating");
worker.terminate();
