import { register } from "node:module";
import { MessageChannel } from "node:worker_threads";

const { port1, port2 } = new MessageChannel();

const messages = [];
port1.on("message", (msg) => {
  messages.push(msg);
});

register("./hooks-initialize.mjs", {
  parentURL: import.meta.url,
  data: { port: port2 },
  transferList: [port2],
});

const { value } = await import("virtual:tracked");
console.log("value:", value);

// Give messages time to arrive via the port
await new Promise((resolve) => setTimeout(resolve, 10));
port1.close();

for (const msg of messages) {
  console.log("message:", msg);
}
