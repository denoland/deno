// Set exit code to some value, we'll ensure that `Deno.exit()` and
// setting exit code in worker context is a no-op. `Deno.exit()` should close
// the worker without calling an overridden `self.close` property.

// @ts-ignore Deno[Deno.internal].core doesn't have type-defs
Deno[Deno.internal].core.ops.op_set_exit_code(21);

const worker = new Worker(
  import.meta.resolve("./op_exit_op_set_exit_code_worker.js"),
  { type: "module" },
);

worker.onmessage = (event) => {
  console.log(event.data);
  Deno.exit(1);
};
worker.onerror = (event) => {
  console.log(event.message);
  Deno.exit(1);
};
worker.onmessageerror = () => {
  console.log("messageerror");
  Deno.exit(1);
};
worker.postMessage("go");
