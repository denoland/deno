// Test Pipe.prototype.open() with invalid fd
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { Pipe, constants: PipeConstants } = require("internal/test/binding")
  .internalBinding("pipe_wrap");

const pipe = new Pipe(PipeConstants.SOCKET);
const result = pipe.open(-1);

// Should return a non-zero error code for invalid fd
console.log(`open(-1) returned: ${result}`);
if (result !== 0) {
  console.log("PASS: Invalid fd returns error code");
} else {
  console.log("FAIL: Invalid fd should return error code");
  Deno.exit(1);
}
