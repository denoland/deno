import { deferred } from "../../../test_util/std/async/deferred.ts";

const worker = new Worker(
  new URL("op_exit_op_set_exit_code_worker.js", import.meta.url).href,
  { type: "module" },
);

const promise1 = deferred();
worker.onmessage = (e) => {
  if (e.data != "ok") {
    promise1.reject("not ok");
  }
  promise1.resolve();
};
await promise1;
worker.terminate();
