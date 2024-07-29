import { MessageChannel, Worker } from "node:worker_threads";

const { port1, port2 } = new MessageChannel();
const worker = new Worker(
  import.meta.resolve("./message_port_transfer1.mjs"),
);
// Send the port directly after the worker is created
worker.postMessage(port2, [port2]);
// Send a message to the worker using the transferred port
port1.postMessage("Hello from main thread!");
worker.on("message", (message) => {
  console.log("Received message from worker:", message);
  worker.terminate();
});
