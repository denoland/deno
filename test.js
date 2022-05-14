const callback = Deno.core.opSync("op_ffi_register_callback", {
  parameters: [],
  result: "void",
}, console.log);

Deno.core.opSync("test_registered_callback", callback)

