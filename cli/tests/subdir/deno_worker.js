if (self.name !== "denoWorker") {
  throw Error(`Bad worker name: ${self.name}, expected "denoWorker"`);
}

onmessage = async function (e) {
  if (typeof self.Deno === "undefined") {
    throw new Error("Deno namespace not available in worker");
  }

  const readP = await Deno.permissions.query({ name: "read" });
  const writeP = await Deno.permissions.query({ name: "write" });

  if (readP.state !== "granted") {
    throw new Error("Bad read permissions");
  }

  if (writeP.state !== "granted") {
    throw new Error("Bad write permissions");
  }
  postMessage(e.data);
};
