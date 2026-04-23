// Main script that spawns multiple workers for inspector testing
const worker1 = new Worker(
  new URL("./worker_target.js", import.meta.url).href,
  {
    type: "module",
    name: "Worker1",
  },
);

const worker2 = new Worker(
  new URL("./worker_target.js", import.meta.url).href,
  {
    type: "module",
    name: "Worker2",
  },
);

let readyCount = 0;

worker1.onmessage = (e) => {
  console.log("Worker1 message:", e.data);
  if (e.data === "worker_ready") {
    readyCount++;
    if (readyCount === 2) console.log("all_workers_ready");
  }
};

worker2.onmessage = (e) => {
  console.log("Worker2 message:", e.data);
  if (e.data === "worker_ready") {
    readyCount++;
    if (readyCount === 2) console.log("all_workers_ready");
  }
};

worker1.onerror = console.error;
worker2.onerror = console.error;

// Keep alive until debugger disconnects
await new Promise(() => {});
