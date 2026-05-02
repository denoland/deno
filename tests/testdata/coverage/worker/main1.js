const worker = new Worker(import.meta.resolve("./worker1.js"), {
  type: "module",
});
const p = Promise.withResolvers();
worker.onmessage = (e) => {
  p.resolve(e);
};
worker.onerror = (e) => {
  p.reject(e);
};
worker.postMessage("hello main1");
await p.promise;
await new Promise((resolve) => setTimeout(resolve, 1_000));
