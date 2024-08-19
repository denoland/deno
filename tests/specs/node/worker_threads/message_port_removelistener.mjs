import { MessageChannel } from "node:worker_threads";

const { port1, port2 } = new MessageChannel();
const listener = (message) => {
  console.log(message);
  port1.off("message", listener);
};
port1.on("message", listener);
port2.postMessage("Hello World!");
