import workerThreads from "node:worker_threads";
import { assertEquals } from "jsr:@std/assert";

const { port1, port2 } = new workerThreads.MessageChannel();

const message1 = { hello: "world" };
const message2 = { foo: "bar" };

assertEquals(workerThreads.receiveMessageOnPort(port2), undefined);
port2.start();

port1.postMessage(message1);
port1.postMessage(message2);
assertEquals(workerThreads.receiveMessageOnPort(port2), {
  message: message1,
});
assertEquals(workerThreads.receiveMessageOnPort(port2), {
  message: message2,
});
port1.close();
port2.close();
