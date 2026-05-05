// Set exit code to some value, we'll ensure that `Deno.exit()` and
// setting exit code in worker context is a no-op and is an alias for
// `self.close()`.

Deno.exitCode = 21;

const worker = new Worker(
  import.meta.resolve("./op_exit_op_set_exit_code_worker.js"),
  { type: "module" },
);

worker.postMessage("go");
