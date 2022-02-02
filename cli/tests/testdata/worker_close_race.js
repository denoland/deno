// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// https://github.com/denoland/deno/issues/11416
// Test for a race condition between a worker's `close()` and the main thread's
// `Worker.prototype.terminate()`.

const worker = new Worker(
  new URL("./workers/close_race_worker.js", import.meta.url),
  { type: "module" },
);

worker.onmessage = () => {
  worker.terminate();
};
