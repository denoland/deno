// Listen for messages from the main thread.
onmessage = function(e) {
  console.log(e.data);

  workerClose();
};
