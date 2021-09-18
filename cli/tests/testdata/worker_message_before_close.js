let messagesReceived = 0;

for (let i = 0; i < 4; i++) {
  const worker = new Worker(
    new URL("./workers/message_before_close.js", import.meta.url).href,
    { type: "module", name: String(i) },
  );

  worker.addEventListener("message", () => {
    messagesReceived += 1;

    if (messagesReceived == 4) {
      console.log("received all 4 responses from the workers");
    }
  });

  worker.postMessage(i);
}

globalThis.addEventListener("unload", () => {
  if (messagesReceived !== 4) {
    console.log(
      "received only %d responses from the workers",
      messagesReceived,
    );
  }
});

export {};
