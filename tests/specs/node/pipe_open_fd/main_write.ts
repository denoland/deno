// Test Pipe.open(fd) for writing to a file fd.
// Opens a temp file, registers its fd via Pipe.open(), writes through
// the LibuvStreamWrap machinery, and verifies the data was written.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const fs = require("fs");
const path = require("path");
const os = require("os");

const tmpFile = path.join(os.tmpdir(), `deno_pipe_open_test_${process.pid}.txt`);
const fd = fs.openSync(tmpFile, "w");

// Use internal binding to test Pipe.open directly
const { Pipe, socketType } = require("internal_binding/pipe_wrap");
const pipe = new Pipe(socketType.SOCKET);
const err = pipe.open(fd);
console.log("open result:", err);

// Write through the pipe's stream interface
const { WriteWrap } = require("internal_binding/stream_wrap");
const req = new WriteWrap();
const data = new TextEncoder().encode("hello from pipe.open\n");
req.oncomplete = function (status: number) {
  console.log("write status:", status);
  pipe.close(() => {
    // Verify the data was written
    const content = fs.readFileSync(tmpFile, "utf8");
    console.log("content:", content.trim());
    fs.unlinkSync(tmpFile);
  });
};
req.handle = pipe;
pipe.writeBuffer(req, data);
