// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// Test that closing a worker which has living child workers will automatically
// close the children.

console.log("Starting the main thread");

const worker = new Worker(
  new URL("./workers/close_nested_parent.js", import.meta.url),
  { type: "module" },
);

setTimeout(() => {
  console.log("About to close");
  worker.postMessage({});

  // Keep the process running for another two seconds, to make sure there's no
  // output from the child worker.
  setTimeout(() => {}, 2000);
}, 1000);
