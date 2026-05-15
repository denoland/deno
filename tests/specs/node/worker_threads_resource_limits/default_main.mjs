import { Worker } from "node:worker_threads";

// No resourceLimits specified - should get defaults
const worker = new Worker(new URL("./worker.mjs", import.meta.url));

worker.on("message", (msg) => {
  console.log("resourceLimits:", JSON.stringify(msg));
});

worker.on("exit", (code) => {
  console.log("exit:", code);
});
