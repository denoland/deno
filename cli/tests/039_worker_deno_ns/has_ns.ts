onmessage = (msg): void => {
  if (msg.data === "START") {
    postMessage("has_ns.ts: is self.Deno available: " + !!self.Deno);
  } else {
    const worker = new Worker("./maybe_ns.ts", { type: "module", deno: true });
    worker.onmessage = (msg): void => {
      postMessage("[SPAWNED BY has_ns.ts] " + msg.data);
    };
  }
};
