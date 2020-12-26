self.onmessage = (ev: MessageEvent) => {
  console.log("received in worker", ev.data);
  self.postMessage(undefined);
  console.log("posted from worker");
};