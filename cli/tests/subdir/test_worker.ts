if (self.name !== "tsWorker") {
  throw Error(`Invalid worker name: ${self.name}, expected tsWorker`);
}

onmessage = function(e): void {
  console.log(e.data);

  postMessage(e.data);

  close();
};
