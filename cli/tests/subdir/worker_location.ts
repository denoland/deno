onmessage = function (): void {
  postMessage(
    `${location.href}, ${location instanceof WorkerLocation}`,
  );
  close();
};
