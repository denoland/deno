// Verify that Pipe.open(fd) takes ownership of the fd, matching Node.js's
// uv_pipe_open() behavior: closing the pipe also closes the original fd.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const fs = require("fs");

const { Pipe, constants: PipeConstants, createPipe } = require(
  "internal/test/binding",
).internalBinding("pipe_wrap");

const [readFd, writeFd] = createPipe();

// Wrap the read end in a Pipe handle (takes ownership of readFd)
const pipe = new Pipe(PipeConstants.SOCKET);
const err = pipe.open(readFd);
if (err !== 0) {
  console.log("FAIL: Pipe.open() returned", err);
  Deno.exit(1);
}
console.log("Pipe.open() succeeded");

// Close the pipe handle -- this should close the underlying fd too
pipe.close(() => {
  // Verify the fd was closed (matching Node.js uv_pipe_open ownership)
  try {
    fs.fstatSync(readFd);
    console.log("FAIL: fd should have been closed");
  } catch (e: any) {
    if (e.code === "EBADF") {
      console.log("PASS: fd was closed with the pipe (ownership transferred)");
    } else {
      console.log("FAIL: unexpected error:", e.message);
    }
  }

  // Clean up write end
  try {
    fs.closeSync(writeFd);
  } catch {}
});
