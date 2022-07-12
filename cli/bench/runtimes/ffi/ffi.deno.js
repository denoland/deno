const libName = new URL("./ffi.dylib", import.meta.url);
// Open library and define exported symbols
const {
  symbols: {
    add,
    noop,
  }
} = Deno.dlopen(
  libName,
  {
    "add": { parameters: ["isize", "isize"], result: "isize" },
    "noop": { parameters: [], result: "void" },
  },
);

{
  Deno.bench("add(50, 51)", () => {
    add(50, 51);
  });
}

{
  Deno.bench("noop()", () => {
    noop();
  });
}

if (add(50, 51) !== 101n) {
  throw new Error("add(50, 51) !== 101");
}
