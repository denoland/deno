if (self.name !== "denoWorker") {
  throw Error(`Bad worker name: ${self.name}, expected "denoWorker"`);
}

onmessage = function (e) {
  if (typeof self.Deno === "undefined") {
    throw new Error("Deno namespace not available in worker");
  }
  postMessage(e.data);
};
