// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// Test that the panic in https://github.com/denoland/deno/issues/11342 does not
// happen when calling worker.terminate() after fixing
// https://github.com/denoland/deno/issues/13705

function getCodeBlobUrl(code) {
  const blob = new Blob([code], { type: "text/javascript" });
  return URL.createObjectURL(blob);
}

const WORKER2 = getCodeBlobUrl(`
  console.log("Worker 2");
  self.postMessage(undefined);

  // We sleep synchronously for slightly under 2 seconds in order to make sure
  // that worker 1 has closed, and that this worker's thread finishes normally
  // rather than being killed (which happens 2 seconds after calling terminate).
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1800);
  console.log("Finished sleeping in worker 2");
`);

const WORKER1 = getCodeBlobUrl(`
  console.log("Worker 1");
  const worker = new Worker(${JSON.stringify(WORKER2)}, { type: "module" });

  worker.addEventListener("message", () => {
    console.log("Terminating");
    worker.terminate();
    self.close();
  });
`);

new Worker(WORKER1, { type: "module" });

// Don't kill the process before worker 2 is finished.
setTimeout(() => {}, 3000);
