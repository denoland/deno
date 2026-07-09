// Regression test for https://github.com/denoland/deno/issues/34399
// When a library (e.g. ts-node) has put stdin into raw mode, the
// permission prompt's line-buffered `read_line` would hang forever
// because Enter delivers `\r` (no `\n`) and ECHO is off.
//
// Verify that the prompt instead bails out with a clear message and
// denies the permission.
Deno.stdin.setRaw(true);
const status = Deno.permissions.requestSync({ name: "env", variable: "FOO" });
console.log("STATUS:", status.state);
