onmessage = function(e) {
  console.log(e.data);

  throw new Error("test error :))))");
  postMessage(e.data);

  workerClose();
};
