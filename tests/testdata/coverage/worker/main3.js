const worker = new Worker(import.meta.resolve("./worker3.js"), {
  type: "module",
});
const p = Promise.withResolvers();
worker.onmessage = (e) => {
  p.resolve(e);
};
worker.onerror = (e) => {
  p.reject(e);
};
worker.postMessage("hello main3");
let rejected = false;
try {
  await p.promise;
} catch {
  rejected = true;
}
if (!rejected) {
  throw new Error("Expected promise to reject");
}

globalThis.onunhandledrejection = (e) => {
  e.preventDefault();
};
