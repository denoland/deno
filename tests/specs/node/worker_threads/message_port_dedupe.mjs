// Regression test: MessagePort.on('message', handler) should deduplicate
// identical listeners (EventTarget-style), matching Node.js behavior.
// See https://github.com/denoland/deno/issues/33373

import { MessageChannel } from "node:worker_threads";

let received = 0;
const { port1, port2 } = new MessageChannel();
const handler = (msg) => {
  received++;
  if (received > 1) {
    console.log("FAIL: handler fired more than once:", msg);
    process.exit(1);
  }
  console.log("got:", msg);
  port1.close();
  port2.close();
};

// Register the same handler four times. In Node.js this dedupes and the
// handler fires exactly once.
port1.on("message", handler);
port1.on("message", handler);
port1.on("message", handler);
port1.on("message", handler);

port2.postMessage("hello");

// Wait a tick to ensure no further deliveries.
setTimeout(() => {
  if (received !== 1) {
    console.log("FAIL: expected 1 delivery, got", received);
    process.exit(1);
  }
  console.log("ok");
}, 50);
