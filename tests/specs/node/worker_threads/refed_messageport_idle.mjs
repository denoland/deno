// Regression test for https://github.com/denoland/deno/issues/23169
//
// A worker that holds a "refed" transferable object (here a MessagePort it
// listens on) must stay alive while idle and waiting for work, instead of
// being terminated on idle. The main thread deliberately stays idle for a
// while before sending work over the dedicated port; if the worker were
// terminated on idle the message would never be delivered and this test
// would hang.
//
// All output is produced on the main thread so the expected output is
// deterministic (worker stdout could otherwise interleave nondeterministically
// across threads).
import {
  isMainThread,
  MessageChannel,
  parentPort,
  Worker,
  workerData,
} from "node:worker_threads";

if (isMainThread) {
  const { port1, port2 } = new MessageChannel();
  const worker = new Worker(import.meta.filename, {
    workerData: port2,
    transferList: [port2],
  });

  // Wait for the worker to finish setup and go idle, holding only its refed
  // MessagePort.
  await new Promise((resolve) => worker.once("message", resolve));
  console.log("main: worker is idle, waiting before sending work");

  port1.on("message", (msg) => {
    console.log("main: got reply:", msg);
    port1.close();
    worker.terminate();
  });

  // Stay idle long enough that a worker terminated-on-idle would already be
  // gone, then send the work item.
  setTimeout(() => {
    console.log("main: sending work after idle period");
    port1.postMessage("work");
  }, 1000);
} else {
  const port = workerData;
  port.on("message", (msg) => {
    port.postMessage("done:" + msg);
    // Now that the work is done, drop the ref so the worker can exit even
    // though the main thread keeps the channel open a little longer.
    port.unref();
  });
  // Signal readiness over parentPort, then go idle. The only thing keeping
  // this worker alive is the refed MessagePort above.
  parentPort.postMessage("ready");
}
