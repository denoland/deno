import { fork } from "node:child_process";
import net from "node:net";
import process from "node:process";

const serialization = process.argv[2] === "advanced" ? "advanced" : "json";

if (process.argv[3] === "child") {
  // Child receives a listening net.Server from the parent, accepts one
  // connection on it, echoes a greeting back, then exits.
  process.on("message", (msg, server) => {
    if (!server || typeof server.address !== "function") {
      process.send({ error: "no server handle" });
      return;
    }
    server.on("connection", (socket) => {
      socket.setEncoding("utf8");
      socket.once("data", (data) => {
        socket.end(`child-server-echo:${data.trim()}\n`);
        server.close();
        process.send({ done: true });
      });
    });
    // Tell the parent we're listening on the inherited fd. By the time the
    // parent receives this, NODE_HANDLE_ACK has already been processed so
    // the parent's server has been closed (closeAfterSend semantics).
    process.send({ ready: true });
  });
  process.on("disconnect", () => process.exit(0));
} else {
  let echoReceived = false;
  let childDone = false;
  let port = 0;

  const server = net.createServer();
  server.listen(0, "127.0.0.1", () => {
    port = server.address().port;

    const child = fork(new URL("./server_main.mjs", import.meta.url).pathname, [
      serialization,
      "child",
    ], { serialization });

    child.on("error", (e) => {
      console.error("child error:", e);
      process.exit(1);
    });
    child.on("message", (msg) => {
      if (msg && msg.error) {
        console.error("child error:", msg.error);
        process.exit(1);
      }
      if (msg && msg.ready) {
        const client = net.connect(port, "127.0.0.1", () => {
          client.write("hello\n");
        });
        client.setEncoding("utf8");
        client.on("data", (d) => {
          console.log(`client got: ${d.trim()}`);
          echoReceived = true;
          maybeFinish();
        });
        return;
      }
      if (msg && msg.done) {
        childDone = true;
        maybeFinish();
      }
    });

    child.send({ kind: "server" }, server, (err) => {
      if (err) {
        console.error("send err:", err);
        process.exit(1);
      }
    });
  });

  function maybeFinish() {
    if (childDone && echoReceived) {
      console.log("ok");
      process.exit(0);
    }
  }

  setTimeout(() => {
    console.error("timeout");
    process.exit(2);
  }, 5000);
}
