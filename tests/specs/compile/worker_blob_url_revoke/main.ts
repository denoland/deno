// Revoking the object URL immediately after constructing the worker (before
// the worker has loaded its module) must not break the worker: the blob is
// captured for the in-flight load. Guards against a future regression where
// the loader lazily looks up an already-revoked blob.
const workerCode = `
  self.onmessage = (e) => {
    self.postMessage(e.data.numbers.reduce((s, n) => s + n, 0));
  };
  self.postMessage("ready");
`;

const blob = new Blob([workerCode], { type: "application/javascript" });
const blobUrl = URL.createObjectURL(blob);
const worker = new Worker(blobUrl, { type: "module" });
URL.revokeObjectURL(blobUrl);

const { promise, resolve } = Promise.withResolvers<void>();

worker.onmessage = (e) => {
  if (e.data === "ready") {
    worker.postMessage({ numbers: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] });
  } else {
    console.log("result:", e.data);
    worker.terminate();
    resolve();
  }
};

await promise;
