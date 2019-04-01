onmessage = function(e) {
  console.log(e.data);

  postMessage(e.data);

  workerClose();
};
