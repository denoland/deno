import { Worker } from "node:worker_threads";

const worker = new Worker(
  new URL("./node_terminate_worker.mjs", import.meta.url),
);
const ready = Promise.withResolvers();

worker.on("message", (message) => {
  if (message === "ready") {
    ready.resolve();
  }
});
worker.on("error", (error) => {
  ready.reject(error);
});

await ready.promise;
const exitCode = await worker.terminate();

if (exitCode !== 1) {
  throw new Error(`Expected worker termination exit code 1, got ${exitCode}`);
}
