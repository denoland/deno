// Test Pipe.prototype.open(fd) happy path using socketpair via FFI
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { Pipe, constants: PipeConstants } = require("internal/test/binding")
  .internalBinding("pipe_wrap");

// Use FFI to create a socketpair
const libName = Deno.build.os === "darwin" ? "libSystem.B.dylib" : "libc.so.6";
const libc = Deno.dlopen(libName, {
  socketpair: { parameters: ["i32", "i32", "i32", "buffer"], result: "i32" },
  close: { parameters: ["i32"], result: "i32" },
});

const AF_UNIX = 1;
const SOCK_STREAM = 1;
const fds = new Int32Array(2);

const result = libc.symbols.socketpair(AF_UNIX, SOCK_STREAM, 0, fds);
if (result !== 0) {
  console.log("FAIL: socketpair failed");
  Deno.exit(1);
}

const fd0 = fds[0];
const fd1 = fds[1];
console.log(`Created socketpair: fd ${fd0} <-> fd ${fd1}`);

// Test Pipe.prototype.open() with a valid fd
const pipe = new Pipe(PipeConstants.SOCKET);
const openResult = pipe.open(fd0);
console.log(`Pipe.open(${fd0}) returned: ${openResult}`);

if (openResult === 0) {
  console.log("PASS: Pipe.open() succeeded with valid fd");
} else {
  console.log("FAIL: Pipe.open() should return 0 for valid fd");
  libc.symbols.close(fd1);
  libc.close();
  Deno.exit(1);
}

// Clean up the other fd (fd0 is now owned by the Pipe)
libc.symbols.close(fd1);
libc.close();
