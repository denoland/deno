const worker = new Worker(import.meta.resolve("./worker_target.ts"), {
  type: "module",
  env: { IS_WORKER: "true" },
} as unknown as WorkerOptions);
worker.onmessage = (e) => {
  console.log(e.data);
  worker.terminate();
};
