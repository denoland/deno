import module from "node:module";

// The path is intentionally left un-normalized. When passed through a
// `registerHooks` `load` hook it reaches `op_require_read_file` without the
// normalization that a regular `require` would apply, exercising the BYONM
// permission bypass (GHSA-prvv-4rg3-43v6).
const TARGET_FILE = "./node_modules/../secret.json";

module.registerHooks({
  resolve(_spec, _context, _next) {
    return { url: "file://node_modules", shortCircuit: true };
  },
  load(_url, _context, nextLoad) {
    const { source } = nextLoad(TARGET_FILE);
    console.log("ERROR: should not have loaded:", source);
    Deno.exit(0);
  },
});

module._load("foobar", null, false);
