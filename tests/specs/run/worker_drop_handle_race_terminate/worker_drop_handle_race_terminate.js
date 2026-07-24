// Copyright 2018-2026 the Deno authors. MIT license.

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

  // Keep module evaluation blocked long enough for terminate() to interrupt it.
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1800);
  console.log("Worker 2 was not terminated");
`);

const WORKER1 = getCodeBlobUrl(`
  console.log("Worker 1");
  const worker = new Worker(${JSON.stringify(WORKER2)}, { type: "module" });

  worker.addEventListener("message", () => {
    console.log("Terminating");
    worker.terminate();
    self.postMessage(undefined);
    self.close();
  });
`);

const worker = new Worker(WORKER1, { type: "module" });

await new Promise((resolve) => {
  worker.addEventListener("message", resolve, { once: true });
});

// Keep the process alive long enough to observe a missed interrupt.
setTimeout(() => {}, 3000);
