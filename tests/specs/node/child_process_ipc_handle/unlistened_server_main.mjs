import { fork } from "node:child_process";
import net from "node:net";
import process from "node:process";

const serialization = process.argv[2] === "advanced" ? "advanced" : "json";

if (process.argv[3] === "child") {
  // Mirrors the issue repro: the child only inspects the message, not the
  // (absent) handle.
  process.on("message", (msg, handle) => {
    if (msg === "server") {
      console.log(`child got message, handle=${handle === undefined}`);
    }
    process.send({ done: true });
  });
  process.on("disconnect", () => process.exit(0));
} else {
  const child = fork(
    new URL("./unlistened_server_main.mjs", import.meta.url).pathname,
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

  // The server has never started listening, so it has no underlying handle.
  // Node strips the handle and delivers the plain message; we must match.
  const server = net.createServer();
  child.send("server", server, (err) => {
    if (err) {
      console.error("send err:", err);
      process.exit(1);
    }
  });

  setTimeout(() => {
    console.error("timeout");
    process.exit(2);
  }, 5000);
}
