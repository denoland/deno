import { isMainThread, Worker } from "node:worker_threads";

if (isMainThread) {
  // This re-loads the current file inside a Worker instance.
  const w = new Worker(import.meta.filename);
} else {
  console.log("Inside Worker!");
  console.log(isMainThread); // Prints 'false'.
}
