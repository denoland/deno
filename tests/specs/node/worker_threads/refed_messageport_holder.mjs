// Regression test for https://github.com/denoland/deno/issues/23169
//
// The issue's "minimum number of workers always created" / "idle and waiting
// for work" use case: a worker is kept alive purely because it holds a refed
// transferable MessagePort, even though it has not (yet) registered any
// "message" listener and is otherwise idle.
//
// This isolates the runtime idle-termination check (`hasMessageEventListener`
// in runtime/js/99_main.js) which consults `messagePort.refedMessagePortsCount`.
// If that count is not observed live, the worker is incorrectly terminated on
// idle and the "exit" event fires before the idle window elapses.
import {
  isMainThread,
  MessageChannel,
  Worker,
  workerData,
} from "node:worker_threads";

if (isMainThread) {
  const { port1, port2 } = new MessageChannel();
  // Keep a reference to port1 so the channel isn't torn down on our side.
  globalThis.__port1 = port1;
  const worker = new Worker(import.meta.filename, {
    workerData: port2,
    transferList: [port2],
  });

  let aliveConfirmed = false;
  worker.on("exit", () => {
    if (!aliveConfirmed) {
      console.log("FAIL: worker was terminated on idle");
      Deno.exit(1);
    }
  });

  // If the worker survives the idle window, the refed port kept it alive.
  setTimeout(() => {
    aliveConfirmed = true;
    console.log("worker stayed alive while idle holding a refed MessagePort");
    worker.terminate();
  }, 1000);
} else {
  const port = workerData;
  // Hold the port refed but register no "message" listener and don't listen
  // on parentPort. The refed MessagePort is the only thing keeping this
  // worker alive.
  port.ref();
}
