import "./worker.ts";

console.log("Starting worker");
const worker = new Worker(
  new URL("./worker.ts", import.meta.url),
  { type: "module" },
);

setTimeout(() => {
  worker.postMessage(42);
}, 500);
