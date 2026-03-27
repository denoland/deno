import process from "node:process";
import { strictEqual } from "node:assert";

// process.argv[0] should be the same as process.execPath (Node.js behavior)
strictEqual(process.argv[0], process.execPath);

// process.argv[1] should be the path to the main module
strictEqual(process.argv[1], new URL(import.meta.url).pathname);

console.log("ok");
