// Set exit code to some value, we'll ensure that `Deno.exit()` and
// setting exit code in worker context is a no-op and is an alias for
// `self.close()`.

// @ts-ignore Deno.core doesn't have type-defs
Deno.core.opSync("op_set_exit_code", 21);

const worker = new Worker(
  new URL("op_exit_op_set_exit_code_worker.js", import.meta.url).href,
  { type: "module" },
);

worker.postMessage("go");
