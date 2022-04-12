// Set exit code
Deno.core.opSync("op_set_exit_code", 42);

self.postMessage("ok");
