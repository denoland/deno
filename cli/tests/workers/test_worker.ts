if (self.name !== "tsWorker") {
  throw Error(`Invalid worker name: ${self.name}, expected tsWorker`);
}

onmessage = function (e): void {
  postMessage(e.data);
  close();
};
