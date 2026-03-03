import { Worker } from "node:worker_threads";

const worker = new Worker(new URL("./oom_worker.mjs", import.meta.url), {
  resourceLimits: {
    maxOldGenerationSizeMb: 64,
  },
});

worker.on("error", (err) => {
  console.log("got error");
});

worker.on("exit", (code) => {
  console.log("exit:", code);
});
