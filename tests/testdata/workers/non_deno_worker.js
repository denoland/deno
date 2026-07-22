onmessage = function (e) {
  if (typeof self.Deno !== "undefined") {
    throw new Error("Deno namespace unexpectedly available in worker");
  }

  postMessage(e.data);
};
