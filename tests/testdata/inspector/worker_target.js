// Worker script for inspector testing
console.log("Worker started");

self.onmessage = (e) => {
  console.log("Worker received:", e.data);
  self.postMessage("pong: " + e.data);
};

// Signal ready
self.postMessage("worker_ready");
