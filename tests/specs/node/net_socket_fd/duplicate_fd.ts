// Regression test: opening a file with fs.openSync() and then trying to
// TCP.open() the same fd should be rejected by the FdTable guard, matching
// Pipe.open(). Without the guard, TCPWrap.open() would blindly adopt any
// descriptor already tracked by Deno.
import { createRequire } from "node:module";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";

const require = createRequire(import.meta.url);
const { TCP, constants: TCPConstants } = require("internal/test/binding")
  .internalBinding("tcp_wrap");

// Create a temp file and open it via fs.openSync (registers in the FdTable)
const tmpFile = path.join(os.tmpdir(), `deno-tcp-dup-test-${process.pid}`);
fs.writeFileSync(tmpFile, "test data");
const fd = fs.openSync(tmpFile, "r");
console.log(`fs.openSync returned fd: ${fd}`);

// TCP.open() on the same fd should fail because it is already registered
const tcp = new TCP(TCPConstants.SOCKET);
const result = tcp.open(fd);
console.log(`TCP.open(${fd}) returned: ${result}`);

if (result !== 0) {
  console.log("PASS: TCP.open() correctly rejected already-registered fd");
} else {
  console.log("FAIL: TCP.open() should reject already-registered fd");
}

// Clean up
fs.closeSync(fd);
fs.unlinkSync(tmpFile);
