onmessage = function(e): void {
  if (e.data === "trigger error") {
    console.log("triggering error");
    throw new Error("error from test_worker.ts");
  }

  console.log(e.data);

  postMessage(e.data);

  if (e.data === "exit") {
    workerClose();
  }
};
