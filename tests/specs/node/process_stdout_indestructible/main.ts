// Regression test for https://github.com/denoland/deno/issues/32513
// Verifies that process.stdout survives destroy() calls.
// Libraries like mute-stream (used by @inquirer/prompts) call destroy()
// on process.stdout, which should not actually close the underlying resource.
import process from "node:process";

console.log("before");
process.stdout.destroy();
console.log("after");

setTimeout(() => {
  console.log("from timeout");
}, 100);
