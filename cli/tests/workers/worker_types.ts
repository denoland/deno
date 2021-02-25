// deno-lint-ignore require-await
self.onmessage = async (_msg: MessageEvent) => {
  self.postMessage("hello");
};
