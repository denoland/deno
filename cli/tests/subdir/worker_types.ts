self.onmessage = async (msg: MessageEvent) => {
  self.postMessage("hello");
};
