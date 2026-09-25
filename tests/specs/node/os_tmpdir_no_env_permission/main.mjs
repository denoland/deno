// Node's os.tmpdir() does not require any permission to read the environment.
// In Deno reading env requires --allow-env; tmpdir() must still work (falling
// back to the default) when the permission has not been granted, instead of
// throwing NotCapable. Regression test for https://github.com/denoland/deno/issues/17949
import os from "node:os";

const tmp = os.tmpdir();
if (typeof tmp !== "string" || tmp.length === 0) {
  throw new Error(`unexpected tmpdir: ${tmp}`);
}
console.log("ok");
