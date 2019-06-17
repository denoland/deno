postMessage("ts worker init");

let thrown = false;

onmessage = (e): void => {
  console.log("ts worker received message:", e.data);

  if (!thrown) {
    thrown = true;
    throw Error("error from ts worker");
  }

  console.log("end ts worker");
  workerClose();
};
