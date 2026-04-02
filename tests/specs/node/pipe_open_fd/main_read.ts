// Test Pipe.open(fd) for reading from a file fd.
// Creates a temp file with known content, opens it for reading,
// and reads through the pipe stream.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const fs = require("fs");
const path = require("path");
const os = require("os");

const tmpFile = path.join(os.tmpdir(), `deno_pipe_read_test_${process.pid}.txt`);
fs.writeFileSync(tmpFile, "data from file\n");

const fd = fs.openSync(tmpFile, "r");

const { Pipe, socketType } = require("internal_binding/pipe_wrap");
const pipe = new Pipe(socketType.SOCKET);
const err = pipe.open(fd);
console.log("open result:", err);

// Read through the pipe by starting the read loop
pipe.onread = function (buf: Uint8Array, nread: number) {
  if (nread > 0) {
    const text = new TextDecoder().decode(buf.subarray(0, nread));
    console.log("read:", text.trim());
  } else {
    console.log("eof");
    pipe.close(() => {
      fs.unlinkSync(tmpFile);
    });
  }
};
pipe.readStart();
