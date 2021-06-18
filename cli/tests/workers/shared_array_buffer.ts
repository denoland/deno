self.postMessage("ready");

globalThis.addEventListener("message", (e) => {
  const buffer = new Uint8Array(e.data);
  buffer[0] = 1;
  self.postMessage("done");
});
