// Regression test for https://github.com/denoland/deno/issues/33366
// After the first client disconnects from a pipe server, the server
// must accept subsequent connections. On Windows this requires re-arming
// the named pipe instance (calling mark_ready on the waker after
// creating a new server in uv_pipe_accept).

import { connect, createServer } from "node:net";
import { randomUUID } from "node:crypto";

const suffix = randomUUID().slice(0, 8);
const pipePath = Deno.build.os === "windows"
  ? `\\\\.\\pipe\\deno_test_reaccept_${suffix}`
  : `${Deno.makeTempDirSync()}/reaccept_${suffix}.sock`;

let connectionCount = 0;

const server = createServer((socket) => {
  connectionCount++;
  console.log(`connection ${connectionCount}`);
  socket.on("data", (chunk: Buffer) => {
    console.log(`data: ${chunk.toString().trim()}`);
  });
  socket.on("end", () => {
    socket.end();
  });
});

await new Promise<void>((resolve, reject) => {
  server.once("error", reject);
  server.listen(pipePath, resolve);
});

async function connectAndSend(label: string): Promise<void> {
  const sock = connect(pipePath);
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => {
      sock.destroy();
      reject(new Error(`${label}: connect timeout`));
    }, 5000);
    sock.once("connect", () => {
      clearTimeout(timer);
      resolve();
    });
    sock.once("error", (e: Error) => {
      clearTimeout(timer);
      reject(e);
    });
  });
  sock.write(`hello from ${label}`);
  // Wait for server to process the data before closing.
  await new Promise((r) => setTimeout(r, 100));
  sock.destroy();
  // Wait for server-side close handling to complete.
  await new Promise((r) => setTimeout(r, 100));
}

await connectAndSend("A");
await connectAndSend("B");
await connectAndSend("C");

server.close();

console.log(`total connections: ${connectionCount}`);
