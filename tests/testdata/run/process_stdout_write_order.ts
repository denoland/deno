// Regression test for https://github.com/denoland/deno/issues/32473
// Verifies that process.stdout.write() and console.log() produce output
// in the correct order when stdout is a TTY.
import process from "node:process";

process.stdout.write("A\n");
console.log("B");
process.stdout.write("C\n");
console.log("D");
process.stdout.write("E\n");
