// Test Pipe.prototype.open(fd) on Windows using _pipe() via FFI
// to create a real anonymous pipe pair.
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { Pipe, constants: PipeConstants } = require("internal/test/binding")
  .internalBinding("pipe_wrap");

// Use CRT _pipe() to create an anonymous pipe pair
const ucrtbase = Deno.dlopen("ucrtbase.dll", {
  _pipe: { parameters: ["buffer", "u32", "i32"], result: "i32" },
  _close: { parameters: ["i32"], result: "i32" },
});

const fds = new Int32Array(2);
// _O_BINARY = 0x8000
const result = ucrtbase.symbols._pipe(fds, 4096, 0x8000);
if (result !== 0) {
  console.log("FAIL: _pipe() failed");
  Deno.exit(1);
}

const readFd = fds[0];
const writeFd = fds[1];
console.log(`Created pipe: read fd ${readFd}, write fd ${writeFd}`);

// Test Pipe.prototype.open() with the read end
const pipe = new Pipe(PipeConstants.SOCKET);
const openResult = pipe.open(readFd);
console.log(`Pipe.open(${readFd}) returned: ${openResult}`);

if (openResult === 0) {
  console.log("PASS: Pipe.open() succeeded with valid fd");
} else {
  console.log("FAIL: Pipe.open() should return 0 for valid fd");
}

// Clean up the write end (read end is owned by the Pipe)
ucrtbase.symbols._close(writeFd);
ucrtbase.close();
