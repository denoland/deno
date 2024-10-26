if (self.name !== "tsWorker") {
  throw Error(`Invalid worker name: ${self.name}, expected tsWorker`);
}

onmessage = function (e) {
  postMessage(e.data);
  close();
};
