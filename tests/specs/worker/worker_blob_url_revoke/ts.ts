// TypeScript blob module workers must still transpile after the captured-blob
// fix, even when the object URL is revoked synchronously after construction.
const workerCode = `
  const value: number = 42;
  self.onmessage = (e: MessageEvent): void => {
    const received: number = e.data;
    self.postMessage(received + value);
  };
`;

for (let i = 0; i < 10; i++) {
  const blob = new Blob([workerCode], { type: "application/typescript" });
  const blobUrl = URL.createObjectURL(blob);
  const worker = new Worker(blobUrl, { type: "module" });
  URL.revokeObjectURL(blobUrl);

  await new Promise<void>((resolve, reject) => {
    worker.onmessage = (e) => {
      console.log(e.data);
      worker.terminate();
      resolve();
    };
    worker.onerror = (e) => {
      e.preventDefault();
      reject(e.error ?? new Error(e.message));
    };
    worker.postMessage(i);
  });
}
