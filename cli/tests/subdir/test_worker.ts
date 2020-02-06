if (self.name !== "tsWorker") {
  throw Error(`Invalid worker name: ${self.name}, expected tsWorker`);
}

onmessage = function(e): void {
  console.log("calling onmessage ts!");
  postMessage(e.data);
  console.log("calling close in ts!");
  close();
};
