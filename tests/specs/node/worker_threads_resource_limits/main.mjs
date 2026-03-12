import { Worker } from "node:worker_threads";

const worker = new Worker(new URL("./worker.mjs", import.meta.url), {
  resourceLimits: {
    maxOldGenerationSizeMb: 128,
    maxYoungGenerationSizeMb: 16,
    codeRangeSizeMb: 64,
    stackSizeMb: 4,
  },
});

worker.on("message", (msg) => {
  console.log("resourceLimits:", JSON.stringify(msg));
});

worker.on("error", (err) => {
  console.error("error:", err.message);
});

worker.on("exit", (code) => {
  console.log("exit:", code);
});
