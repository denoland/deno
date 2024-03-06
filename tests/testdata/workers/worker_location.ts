onmessage = function () {
  postMessage(
    `${location.href}, ${location instanceof WorkerLocation}`,
  );
  close();
};
