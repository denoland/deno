for (let i = 0; i < 4; i++) {
  const worker = new Worker(
    new URL("./workers/message_before_close.js", import.meta.url).href,
    { type: "module", name: String(i) },
  );

  worker.addEventListener("message", (message) => {
    // Only print responses in order after all reception logs.
    setTimeout(() => {
      console.log("response from worker %d received", message.data);
    }, 500 * (i + 1));
  });

  worker.postMessage(i);
}

export {};
