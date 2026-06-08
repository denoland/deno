import { fork } from "node:child_process";
import net from "node:net";
import process from "node:process";
import os from "node:os";
import path from "node:path";

const serialization = process.argv[2] === "advanced" ? "advanced" : "json";

if (process.argv[3] === "child") {
  // Child receives a listening unix-socket (Pipe) net.Server from the parent.
  // Before this fix sending it threw "ChildProcess.send with non-TCP
  // net.Server handle"; we now reconstruct a working net.Server around the
  // inherited fd. We don't do a connect round-trip here: closing the parent's
  // server unlinks the socket path (so the path stops routing), which is an
  // inherent property of transferring unix-socket servers, not a Deno bug.
  process.on("message", (msg, server) => {
    const ok = server instanceof net.Server &&
      typeof server.address === "function";
    if (ok) {
      // The inherited fd is a real listening socket we can accept on.
      server.on("connection", (socket) => socket.destroy());
      console.log("child got server");
      server.close();
    } else {
      console.log("child got non-server");
    }
    process.send({ done: true });
  });
  process.on("disconnect", () => process.exit(0));
} else {
  const sockPath = path.join(
    os.tmpdir(),
    `deno-ipc-pipe-${process.pid}.sock`,
  );

  const server = net.createServer();
  server.listen(sockPath, () => {
    const child = fork(
      new URL("./pipe_server_main.mjs", import.meta.url).pathname,
      [serialization, "child"],
      { serialization },
    );

    child.on("error", (e) => {
      console.error("child error:", e);
      process.exit(1);
    });
    child.on("message", (msg) => {
      if (msg && msg.done) {
        child.disconnect();
        console.log("ok");
        process.exit(0);
      }
    });

    child.send({ kind: "pipe-server" }, server, (err) => {
      if (err) {
        console.error("send err:", err);
        process.exit(1);
      }
    });
  });

  setTimeout(() => {
    console.error("timeout");
    process.exit(2);
  }, 5000);
}
