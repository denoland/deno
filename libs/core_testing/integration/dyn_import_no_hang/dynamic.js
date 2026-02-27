// Copyright 2018-2025 the Deno authors. MIT license.
await new Promise((resolve) => {
  // Resolve the promise after one tick of the event loop.
  setTimeout(() => {
    resolve();
  }, 0);
});
console.log("module executed");
