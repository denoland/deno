const { symbols } = Deno.dlopen("target/debug/libtest_ffi.dylib", {
    is_null_ptr: {
        parameters: ["buffer"],
        result: "bool",
    },
})

console.log(symbols.is_null_ptr("hello"))
Deno.bench(
  "pass string",
  () => symbols.is_null_ptr("hello")
)
