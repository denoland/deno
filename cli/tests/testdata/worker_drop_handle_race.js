// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// https://github.com/denoland/deno/issues/11342
// Test for a panic that happens when the main thread's event loop finishes
// running while the worker's event loop is still spinning.

// The exception thrown in the worker will not terminate the worker, but it will
// propagate to the main thread and cause it to exit.
new Worker(
  new URL("./workers/drop_handle_race.js", import.meta.url).href,
  { type: "module" },
);
