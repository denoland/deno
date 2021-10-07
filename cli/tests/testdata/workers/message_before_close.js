self.onmessage = (params) => {
  const workerId = params.data;
  console.log("message received in worker %d", workerId);
  self.postMessage(workerId);
  self.close();
};
