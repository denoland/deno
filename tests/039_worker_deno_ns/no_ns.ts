onmessage = (msg): void => {
  if (msg.data === "START") {
    postMessage("no_ns.ts: is window.Deno available: " + !!window.Deno);
  } else {
    const worker = new Worker("./maybe_ns.ts");
    worker.onmessage = (msg): void => {
      postMessage("[SPAWNED BY no_ns.ts] " + msg.data);
    };
  }
};
