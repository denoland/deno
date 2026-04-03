// Test that fs.writeSync() works on a raw OS fd not registered in Deno's resource table.
// Uses a socketpair via FFI to create a raw fd, then writes to it with fs.writeSync().
import * as fs from "node:fs";

const libName = Deno.build.os === "darwin" ? "libSystem.B.dylib" : "libc.so.6";
const libc = Deno.dlopen(libName, {
  socketpair: { parameters: ["i32", "i32", "i32", "buffer"], result: "i32" },
  read: { parameters: ["i32", "buffer", "usize"], result: "isize" },
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

const writeFd = fds[0];
const readFd = fds[1];

const message = "hello from fs.writeSync\n";
const buf = Buffer.from(message);

// Write using fs.writeSync() — this fd is not in Deno's resource table so it
// will be auto-registered by file_for_fd() (gated by --allow-fd).
const nwritten = fs.writeSync(writeFd, buf, 0, buf.length, null);
if (nwritten !== buf.length) {
  console.log(`FAIL: expected ${buf.length} bytes written, got ${nwritten}`);
  libc.symbols.close(writeFd);
  libc.symbols.close(readFd);
  libc.close();
  Deno.exit(1);
}

// Read back and verify
const readBuf = new Uint8Array(buf.length);
const nread = libc.symbols.read(readFd, readBuf, BigInt(readBuf.length));
if (Number(nread) !== buf.length) {
  console.log(`FAIL: expected to read ${buf.length} bytes, got ${nread}`);
  libc.symbols.close(writeFd);
  libc.symbols.close(readFd);
  libc.close();
  Deno.exit(1);
}

const received = new TextDecoder().decode(readBuf);
if (received !== message) {
  console.log(`FAIL: expected "${message}", got "${received}"`);
  libc.symbols.close(writeFd);
  libc.symbols.close(readFd);
  libc.close();
  Deno.exit(1);
}

console.log("PASS: fs.writeSync() succeeded on raw fd");

libc.symbols.close(writeFd);
libc.symbols.close(readFd);
libc.close();
