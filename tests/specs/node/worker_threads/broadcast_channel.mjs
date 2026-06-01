import { BroadcastChannel, isMainThread, Worker } from "node:worker_threads";

const bc = new BroadcastChannel("hello");

if (isMainThread) {
  bc.onmessage = (event) => {
    console.log(event.data);
    bc.close();
  };
  new Worker(new URL(import.meta.url));
} else {
  bc.postMessage("hello from worker");
  bc.close();
}
