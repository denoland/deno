onmessage = function (): void {
  postMessage(
    [
      self instanceof DedicatedWorkerGlobalScope,
      self instanceof WorkerGlobalScope,
      self instanceof EventTarget,
    ].join(", "),
  );
  close();
};
