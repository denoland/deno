import { Worker } from "node:worker_threads";

// No resourceLimits: the worker isolate's thread must still get the default
// stack size that `resourceLimits.stackSizeMb` reports back to JS.
const worker = new Worker(new URL("./stack_size_worker.mjs", import.meta.url));

worker.on("message", (msg) => {
  console.log("caught RangeError:", msg.caught);
});
