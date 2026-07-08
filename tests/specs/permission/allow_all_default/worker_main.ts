// A worker created with default permissions inherits the parent's allow-all
// default.
const worker = new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
});
const result = await new Promise((resolve) => {
  worker.onmessage = (e) => resolve(e.data);
});
console.log(result);
worker.terminate();
