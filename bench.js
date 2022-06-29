const lib = Deno.dlopen("./target/release/libtest_ffi.dylib", {
  nop: {
    parameters: [],
    result: "void",
  },
  nop_u8: {
    parameters: ["u8"],
    result: "void",
  },
  return_u8: {
    parameters: [],
    result: "u8",
  },
  add_u8: {
    parameters: ["u8", "u8"],
    result: "u8",
  },
}).symbols;

Deno.bench("ffi noop", () => {
  lib.nop();
});

const noop = () => {};

Deno.bench("js noop", () => {
  noop();
});

Deno.bench("noop_u8()", () => {
  lib.nop_u8(100);
});

Deno.bench("return_u8()", () => {
  lib.return_u8();
});

Deno.bench("add_u8()", () => {
  lib.add_u8(1, 2);
});
