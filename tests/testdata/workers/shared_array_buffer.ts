self.postMessage("ready");

globalThis.addEventListener("message", (e) => {
  const bytes1 = new Uint8Array(e.data[0]);
  const bytes2 = new Uint8Array(e.data[1]);
  bytes1[0] = 1;
  bytes2[0] = 2;
  self.postMessage("done");
});
