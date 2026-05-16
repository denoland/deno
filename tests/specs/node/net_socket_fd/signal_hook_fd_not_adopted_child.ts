import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const net = require("net");

function snapshotFds() {
  const fds = new Set<number>();
  for (const entry of Deno.readDirSync("/proc/self/fd")) {
    const fd = Number(entry.name);
    if (Number.isInteger(fd)) {
      fds.add(fd);
    }
  }
  return fds;
}

const before = snapshotFds();
const signalHandler = () => {};
Deno.addSignalListener("SIGTERM", signalHandler);
const after = snapshotFds();

const signalHookFds = [...after].filter((fd) => fd >= 3 && !before.has(fd));
if (signalHookFds.length === 0) {
  throw new Error("failed to observe signal-hook fds");
}

for (const fd of signalHookFds) {
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
