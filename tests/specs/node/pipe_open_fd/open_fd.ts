// Test Pipe.prototype.open(fd) happy path using an anonymous pipe pair
// created by the internal createPipe() helper (works on all platforms).
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { Pipe, constants: PipeConstants, createPipe } = require(
  "internal/test/binding",
).internalBinding("pipe_wrap");

const [readFd, writeFd] = createPipe();
console.log(`Created pipe: read fd ${readFd}, write fd ${writeFd}`);

// Test Pipe.prototype.open() with a valid fd
const pipe = new Pipe(PipeConstants.SOCKET);
const openResult = pipe.open(readFd);
console.log(`Pipe.open(${readFd}) returned: ${openResult}`);

if (openResult === 0) {
  console.log("PASS: Pipe.open() succeeded with valid fd");
} else {
  console.log("FAIL: Pipe.open() should return 0 for valid fd");
  Deno.exit(1);
}
