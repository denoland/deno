// Regression test for https://github.com/denoland/deno/issues/32803
//
// Must be run inside a PTY so process.stdout is a real TTY backed by the
// Rust LibUvStreamWrap (whose write_buffer sets kLastWriteWasAsync = 0).

import net from "node:net";
import path from "node:path";
import process from "node:process";

const errors = [];

process.on("uncaughtException", (err) => {
  errors.push(err);
});

const socketPath = path.join(Deno.makeTempDirSync(), "test.sock");

const server = net.createServer((conn) => {
  conn.resume();
  conn.on("end", () => conn.end());
});

server.listen(socketPath, () => {
  const client = net.connect(socketPath, () => {
    // Write to stdout (TTY → Rust write path → kLastWriteWasAsync=0),
    // then to the pipe (JS write path). Without the fix, the stale
    // kLastWriteWasAsync=0 causes afterWriteDispatched to fire the
    // callback synchronously, then async #write fires it again.
    let pending = 20;
    for (let i = 0; i < 20; i++) {
      process.stdout.write(".");
      client.write(`msg ${i}\n`, () => {
        if (--pending === 0) done();
      });
    }

    function done() {
      client.end(() => {
        server.close(() => {
          process.stdout.write("\n");
          const multiCb = errors.find(
            (e) => e.code === "ERR_MULTIPLE_CALLBACK",
          );
          if (multiCb) {
            console.log("FAIL: ERR_MULTIPLE_CALLBACK");
            process.exit(1);
          }
          console.log("OK");
          process.exit(0);
        });
      });
    }
  });
});
