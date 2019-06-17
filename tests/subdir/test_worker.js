postMessage("js worker init");

let thrown = false;

onmessage = e => {
  console.log("js worker received message:", e.data);

  if (!thrown) {
    thrown = true;
    throw DoesNotExist();
  }

  console.log("end js worker");
  workerClose();
};
