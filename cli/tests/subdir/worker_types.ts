// eslint-disable-next-line require-await
self.onmessage = async (_msg: MessageEvent) => {
  self.postMessage("hello");
};
