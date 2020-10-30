onmessage = function (e): void {
  if (Deno.inspect(1, { colors: false }) != "1") {
    throw new Error("Inspect didn't work.");
  }

  postMessage(e.data);
};
