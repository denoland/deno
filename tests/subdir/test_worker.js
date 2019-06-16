onmessage = function(e) {
  if (e.data === "trigger error") {
    throw new Error("error from test_worker.js");
  }

  console.log(e.data);
  postMessage(e.data);
};

onerror = function(e) {
  console.log("Handled error: ", e.message);
  postMessage(e.message);

  workerClose();
};
