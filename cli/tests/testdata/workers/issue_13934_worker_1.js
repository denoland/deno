// worker1.js
console.log("Worker 1");
new Worker(
    new URL("./worker2.js", import.meta.url).href,
    { type: "module", deno: { namespace: true } }
);