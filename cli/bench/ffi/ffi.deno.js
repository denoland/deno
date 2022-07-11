import { bench, run } from "https://esm.run/mitata";

const libName = `./ffi.dylib`;
// Open library and define exported symbols
const dylib = Deno.dlopen(
  libName,
  {
    "add": { parameters: ["isize", "isize"], result: "isize" },
    "noop": { parameters: [], result: "void" },
  },
);

{
  bench("add(50, 51)", () => {
    dylib.symbols.add(50, 51);
  });
}

{
  bench("noop()", () => {
    dylib.symbols.noop();
  });
}

await run({ collect: false, percentiles: true });
if (dylib.symbols.add(50, 51) !== 101n) {
  throw new Error("add(50, 51) !== 101");
}
