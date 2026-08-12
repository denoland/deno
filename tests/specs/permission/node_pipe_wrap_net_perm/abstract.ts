// Abstract unix sockets (Linux-only, leading NUL) have no filesystem entry,
// so - matching the native `Deno.listen({ transport: "unix" })` path - they
// must be usable with an `--allow-net` grant alone, no filesystem permission
// required. Regular path-based sockets must still be denied without a
// filesystem grant, proving the abstract special case is not over-applied.
//
// This process is granted `--allow-net` but no read/write permissions.

import process from "node:process";

// deno-lint-ignore no-explicit-any
const { Pipe, constants } = (process as any).binding("pipe_wrap");

// bind() on an abstract socket: no filesystem entry, --allow-net suffices.
const ret = new Pipe(constants.SERVER).bind(
  `\0deno-pipewrap-abstract-${process.pid}`,
);
console.log("bind abstract:", ret === 0 ? "PASS" : `FAIL:${ret}`);

// bind() on a path-based socket: still requires the filesystem grant.
try {
  new Pipe(constants.SERVER).bind("/tmp/deno-pipewrap-abstract-fs.sock");
  console.log("bind path: FAIL:allowed");
} catch (e) {
  console.log(
    "bind path:",
    (e as Error).name === "NotCapable" ? "PASS" : `FAIL:${(e as Error).name}`,
  );
}
