onmessage = function(e) {
  console.log(e.data);

  throw new Error("test error :))))");
  postMessage(e.data);

  workerClose();
};

onerror = function(e, a, d, f, h) {
  console.log("caught in worker onerror", e, a, d, f, h);
  return true;
}