self.postMessage("ready");

globalThis.addEventListener("message", (e) => {
  new Uint8Array(e.data)[0] = 1;
});
