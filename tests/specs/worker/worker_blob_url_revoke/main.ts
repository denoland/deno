const workerCode = `
  self.onmessage = (e) => {
    self.postMessage(e.data);
  };
`;

for (let i = 0; i < 10; i++) {
  const blob = new Blob([workerCode], { type: "application/javascript" });
  const blobUrl = URL.createObjectURL(blob);
  const worker = new Worker(blobUrl, { type: "module" });
  URL.revokeObjectURL(blobUrl);

  await new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(() => {
      worker.terminate();
      reject(new Error(`Worker ${i} timed out`));
    }, 10_000);
    worker.onmessage = (e) => {
      clearTimeout(timeout);
      console.log(e.data);
      worker.terminate();
      resolve();
    };
    worker.onerror = (e) => {
      clearTimeout(timeout);
      worker.terminate();
      e.preventDefault();
      reject(e.error ?? new Error(e.message));
    };
    worker.postMessage(i);
  });
}
