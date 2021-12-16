const messagesReceived = new Set();

for (let i = 0; i < 4; i++) {
  const worker = new Worker(
    new URL("./workers/message_before_close.js", import.meta.url).href,
    { type: "module", name: String(i) },
  );

  worker.addEventListener("message", () => {
    messagesReceived.add(i);
    if (messagesReceived.size == 4) {
      console.log("received all 4 responses from the workers");
    }
  });

  worker.postMessage({});
}

globalThis.addEventListener("unload", () => {
  if (messagesReceived.size !== 4) {
    console.log(
      "received only %d responses from the workers",
      messagesReceived.size,
    );
  }
});

export {};
