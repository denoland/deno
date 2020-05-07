onmessage = (msg): void => {
  if (msg.data === "START") {
    postMessage("no_ns.ts: is self.Deno available: " + !!self.Deno);
  } else {
    const worker = new Worker("./maybe_ns.ts", { type: "module" });
    worker.onmessage = (msg): void => {
      postMessage("[SPAWNED BY no_ns.ts] " + msg.data);
    };
  }
};
