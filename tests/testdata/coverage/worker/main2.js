const worker = new Worker(import.meta.resolve("./worker2.js"), {
  type: "module",
});
const p = Promise.withResolvers();
worker.onmessage = (e) => {
  p.resolve(e);
};
worker.onerror = (e) => {
  p.reject(e);
};
worker.postMessage("hello main2");
await p.promise;
worker.terminate();
