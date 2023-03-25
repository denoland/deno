const code = `
addEventListener("message", () => {
  postMessage("pong");
});

const context = new EventTarget();

Object.defineProperty(globalThis, "dispatchEvent", {
  value: context.dispatchEvent.bind(context),
  writable: true,
  enumerable: true,
  configurable: true,
});

postMessage("start");
`;

const blob = new Blob([code], { type: "application/javascript" });

const url = URL.createObjectURL(blob);

const worker = new Worker(url, { type: "module" });

let terminated = false;

worker.addEventListener("message", (evt) => {
  if (evt.data === "start") {
    worker.postMessage("ping");
  } else if (evt.data === "pong") {
    worker.terminate();
    terminated = true;
    console.log("success");
  } else {
    throw new Error("unexpected message from worker");
  }
});

setTimeout(() => {
  if (!terminated) {
    worker.terminate();
    throw new Error("did not receive message from worker in time");
  }
}, 2000);
