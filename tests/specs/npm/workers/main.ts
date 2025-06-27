import { Worker as WorkerThread } from "node:worker_threads";

new Worker(new URL("./worker1.ts", import.meta.url), {
  type: "module",
});
new Worker(new URL("./worker2.ts", import.meta.url), {
  type: "module",
});
new Worker(new URL("./worker3.ts", import.meta.url), {
  type: "module",
});
new WorkerThread(new URL("./worker4.mjs", import.meta.url));
new Worker(new URL("./worker5.mjs", import.meta.url), {
  type: "module",
});
