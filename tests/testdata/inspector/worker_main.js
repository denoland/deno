// Main script that spawns a worker for inspector testing
const worker = new Worker(new URL("./worker_target.js", import.meta.url).href, {
  type: "module",
});

worker.onmessage = (e) => {
  console.log("Main received:", e.data);
};

worker.onerror = (e) => {
  console.error("Worker error:", e.message);
};

// Keep alive until debugger disconnects
await new Promise(() => {});
