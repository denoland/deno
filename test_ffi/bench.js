const lib = Deno.dlopen("/Users/divy/gh/deno/target/release/libtest_ffi.dylib", {
  "add_u32": { parameters: ["u32", "u32"], result: "u32" },
  "nop": { parameters: [], result: "void" },
});

Deno.bench("nop()", () => {
  lib.symbols.nop();
});

Deno.bench("add(1,2)", () => {
  lib.symbols.add_u32(1, 2);
});
