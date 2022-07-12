const libName = new URL("./ffi.dylib", import.meta.url);
// Open library and define exported symbols
const dylib = Deno.dlopen(
  libName,
  {
    "add": { parameters: ["isize", "isize"], result: "isize" },
    "noop": { parameters: [], result: "void" },
  },
);

{
  Deno.bench("add(50, 51)", () => {
    dylib.symbols.add(50, 51);
  });
}

{
  Deno.bench("noop()", () => {
    dylib.symbols.noop();
  });
}

if (dylib.symbols.add(50, 51) !== 101n) {
  throw new Error("add(50, 51) !== 101");
}
