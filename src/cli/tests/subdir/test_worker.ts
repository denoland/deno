onmessage = function(e): void {
  console.log(e.data);

  postMessage(e.data);

  workerClose();
};
