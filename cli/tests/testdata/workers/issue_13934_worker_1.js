// worker1.js
console.log("Worker 1");
new Worker(
  new URL("./issue_13934_worker_2.js", import.meta.url).href,
  { type: "module", deno: { namespace: true } },
);
