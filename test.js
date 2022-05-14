const callback = Deno.core.opSync("op_ffi_register_callback", {
  parameters: [],
  result: "void",
}, function daCallback() {
  console.log("Called");
});

Deno.core.opSync("test_registered_callback", callback);
