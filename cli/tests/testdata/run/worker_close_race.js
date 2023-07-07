// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// https://github.com/denoland/deno/issues/11416
// Test for a race condition between a worker's `close()` and the main thread's
// `Worker.prototype.terminate()`.

const worker = new Worker(
  import.meta.resolve("../workers/close_race_worker.js"),
  { type: "module" },
);

worker.onmessage = () => {
  worker.terminate();
};
