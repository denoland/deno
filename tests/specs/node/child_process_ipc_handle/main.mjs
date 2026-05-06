import { fork } from "node:child_process";
import net from "node:net";
import process from "node:process";

const serialization = process.argv[2] === "advanced" ? "advanced" : "json";

if (process.argv[3] === "child") {
  process.on("message", (msg, handle) => {
    if (!handle) {
      process.send({ error: "no handle" });
      return;
    }
    handle.write(`child-saw:${msg.greeting}\n`);
    handle.end();
    process.send({ done: true });
  });
  process.on("disconnect", () => process.exit(0));
} else {
  let childReply = false;
  let done = false;

  const server = net.createServer((socket) => {
    socket.setEncoding("utf8");
    socket.once("data", (data) => {
      console.log(`parent got: ${data.trim()}`);
      child.send(
        { greeting: data.trim() },
        socket,
        (err) => {
          if (err) {
            console.error("send err:", err);
            process.exit(1);
          }
        },
      );
    });
  });

  const child = fork(new URL("./main.mjs", import.meta.url).pathname, [
    serialization,
    "child",
  ], {
    serialization,
  });

  child.on("message", (msg) => {
    if (msg && msg.error) {
      console.error("child error:", msg.error);
      process.exit(1);
    }
    if (msg && msg.done) done = true;
    maybeFinish();
  });

  child.on("error", (e) => {
    console.error("child error:", e);
    process.exit(1);
  });

  function maybeFinish() {
    if (done && childReply) {
      child.disconnect();
      server.close();
      console.log("ok");
      process.exit(0);
    }
  }

  server.listen(0, "127.0.0.1", () => {
    const { port } = server.address();
    const client = net.connect(port, "127.0.0.1", () => {
      client.write("ping\n");
    });
    client.setEncoding("utf8");
    client.on("data", (d) => {
      console.log(`client got: ${d.trim()}`);
      childReply = true;
      maybeFinish();
    });
  });

  setTimeout(() => {
    console.error("timeout");
    process.exit(2);
  }, 5000);
}
