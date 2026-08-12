// A TypeScript blob module worker must still transpile in the standalone
// (`deno compile`) loader, even when the object URL is revoked synchronously
// after construction. Guards the captured-blob path in `EmbeddedModuleLoader`
// against a regression where the root would be served as raw (un-transpiled)
// TypeScript bytes.
const workerCode = `
  const offset: number = 42;
  self.onmessage = (e: MessageEvent): void => {
    const numbers: number[] = e.data.numbers;
    self.postMessage(numbers.reduce((s: number, n: number) => s + n, offset));
  };
  self.postMessage("ready");
`;

const blob = new Blob([workerCode], { type: "application/typescript" });
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
