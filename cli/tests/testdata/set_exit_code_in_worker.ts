import { deferred } from "../../../test_util/std/async/deferred.ts";

const worker = new Worker(
  new URL("set_exit_code_worker.js", import.meta.url).href,
  { type: "module", deno: { namespace: true } },
);

const promise1 = deferred();
worker.onmessage = (_e) => {
  promise1.resolve();
};
await promise1;
worker.terminate();
