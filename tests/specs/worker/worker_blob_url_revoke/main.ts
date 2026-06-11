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
