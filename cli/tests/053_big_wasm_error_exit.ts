const start = Date.now();
// It turns out that there are chances that instead of never exiting,
// the process might instead be stuck for more than 10 seconds.
const p = Deno.run({
  args: [Deno.execPath(), "-A", "big_wasm_error.ts"],
  stdout: "null",
  stderr: "null"
});
await p.status();
const elapsed = Date.now() - start;
if (elapsed > 10000) {
  throw new Error("Takes too long to run");
}
console.log("OK");
