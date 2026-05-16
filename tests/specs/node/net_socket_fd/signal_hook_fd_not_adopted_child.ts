import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const net = require("net");

const signalHandler = () => {};
Deno.addSignalListener("SIGTERM", signalHandler);

for (let fd = 3; fd < 32; fd++) {
  let socket;
  try {
    socket = new net.Socket({ fd, writable: true });
    socket.on("error", () => {});
    socket.write("x", () => {});
  } catch {
    // Most low fds are not valid sockets or are already owned by Deno.
  } finally {
    socket?.destroy();
  }
}

await new Promise((resolve) => setTimeout(resolve, 100));
Deno.removeSignalListener("SIGTERM", signalHandler);
