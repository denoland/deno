import { bench, group, run } from "mitata";
import { dlopen, suffix } from "bun:ffi";

const libName = `./ffi.dylib`;
const {
  symbols: {
    add,
    noop,
  },
} = dlopen(libName, {
  add: {
    args: ["int32_t", "int32_t"],
    returns: "int32_t",
  },
  noop: {
    args: [],
  },
});

{
  bench("add(50, 51)", () => {
    add(50, 51);
  });
}

{
  bench("noop", () => {
    noop();
  });
}

await run({ collect: false, percentiles: true });
if (add(50, 51) !== 101) {
  throw new Error("add(50, 51) !== 101");
}
