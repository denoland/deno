const plugin = Deno.openPlugin(
  "./../../../target/debug/libtest_native_plugin.so"
);
const op_test_io_async = plugin.ops.op_test_io_async;

console.log(op_test_io_async);
