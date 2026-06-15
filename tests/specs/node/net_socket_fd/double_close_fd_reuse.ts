// Regression test: closing a TCP handle that adopted a fd via TCP.open()
// must remove that fd from the FdTable exactly once. A second close() must
// not re-run the removal, because the OS may have reused the old fd number
// for an unrelated file in the meantime; dropping that entry would close
// the unrelated fd out from under its owner (EBADF).
//
// Repro:
//   1. Open a raw TCP socket fd and adopt it with TCP.open(fd).
//   2. Close the handle (frees the OS fd).
//   3. Open files until the OS reuses the same fd number for a real file.
//   4. Close the handle a second time.
//   5. The reused file fd must still be valid (no EBADF).
import { createRequire } from "node:module";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";

const require = createRequire(import.meta.url);
const { TCP, constants: TCPConstants } = require("internal/test/binding")
  .internalBinding("tcp_wrap");

const libName = Deno.build.os === "darwin" ? "libSystem.B.dylib" : "libc.so.6";
const lib = Deno.dlopen(libName, {
  socket: { parameters: ["i32", "i32", "i32"], result: "i32" },
  close: { parameters: ["i32"], result: "i32" },
});

const AF_INET = 2;
const SOCK_STREAM = 1;

// Create a raw TCP socket fd and adopt it into a TCP handle.
const sockFd = lib.symbols.socket(AF_INET, SOCK_STREAM, 0) as number;
if (sockFd < 0) {
  console.log("FAIL: socket() failed");
  Deno.exit(1);
}

const tcp = new TCP(TCPConstants.SOCKET);
if (tcp.open(sockFd) !== 0) {
  console.log("FAIL: TCP.open(fd) returned error");
  lib.symbols.close(sockFd);
  Deno.exit(1);
}

// First close: drops the UvOwned entry and frees the OS fd. Wait for the
// uv_close callback so the descriptor is actually released before we try
// to make the OS reuse its number.
await new Promise<void>((resolve) => tcp.close(() => resolve()));

// Force the OS to reuse the freed fd number for a real (TableOwned) file.
const tmpFile = path.join(os.tmpdir(), `deno-tcp-doubleclose-${process.pid}`);
fs.writeFileSync(tmpFile, "test data");
const openedFds: number[] = [];
let reusedFd = -1;
for (let i = 0; i < 4096; i++) {
  const fd = fs.openSync(tmpFile, "r");
  openedFds.push(fd);
  if (fd === sockFd) {
    reusedFd = fd;
    break;
  }
}

if (reusedFd === -1) {
  console.log("SKIP: could not force fd reuse");
} else {
  // Second close: with the bug, this synchronously re-removes sockFd from the
  // FdTable and drops the unrelated file's File, closing reusedFd at the OS
  // level. (The libuv side is already closed, so no callback fires here.)
  tcp.close();

  // The reused file fd must still be valid.
  try {
    fs.fstatSync(reusedFd);
    console.log("PASS: reused fd still valid after double close");
  } catch (e) {
    console.log(
      `FAIL: reused fd invalidated by double close: ${(e as Error).message}`,
    );
  }
}

for (const fd of openedFds) {
  try {
    fs.closeSync(fd);
  } catch {
    // The bug already closed reusedFd; ignore.
  }
}
fs.unlinkSync(tmpFile);
lib.close();
