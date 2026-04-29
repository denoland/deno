// Regression test: opening a file with fs.openSync() and then trying to
// Pipe.open() the same fd should be rejected, not silently replace the
// tracked File (which would close the underlying fd on Unix and leak
// CRT bookkeeping on Windows).
import { createRequire } from "node:module";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";

const require = createRequire(import.meta.url);
const { Pipe, constants: PipeConstants } = require("internal/test/binding")
  .internalBinding("pipe_wrap");

// Create a temp file and open it via fs.openSync (registers in NodeFsState)
const tmpFile = path.join(os.tmpdir(), `deno-pipe-dup-test-${process.pid}`);
fs.writeFileSync(tmpFile, "test data");
const fd = fs.openSync(tmpFile, "r");
console.log(`fs.openSync returned fd: ${fd}`);

// Pipe.open() on the same fd should fail because it is already registered
const pipe = new Pipe(PipeConstants.SOCKET);
const result = pipe.open(fd);
console.log(`Pipe.open(${fd}) returned: ${result}`);

if (result !== 0) {
  console.log("PASS: Pipe.open() correctly rejected already-registered fd");
} else {
  console.log("FAIL: Pipe.open() should reject already-registered fd");
}

// Clean up
fs.closeSync(fd);
fs.unlinkSync(tmpFile);
