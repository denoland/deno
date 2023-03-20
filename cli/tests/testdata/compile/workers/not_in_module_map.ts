// This time ./worker.ts is not in the module map, so the worker
// initialization will fail unless worker.js is passed as a side module.

const worker = new Worker(
  new URL("./worker.ts", import.meta.url),
  { type: "module" },
);

setTimeout(() => {
  worker.postMessage(42);
}, 500);
