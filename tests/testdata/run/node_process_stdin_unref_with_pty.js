import process from "node:process";
import util from "node:util";

console.log("START");
globalThis.addEventListener("unload", () => console.log("END"));

const args = util.parseArgs({ options: { unref: { type: "boolean" } } });

// call stdin.unref if --unref is passed
if (args.values.unref) {
  process.stdin.unref();
}

process.stdin.pipe(process.stdout);
