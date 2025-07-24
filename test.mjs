// server.mjs
import { createServer } from "net";
import { TLSSocket } from "tls";
import { readFile } from "fs/promises";
import { unlink } from "fs/promises";
import path from "path";

const SOCKET_PATH = "/tmp/secure.sock";

try {
  await unlink(SOCKET_PATH);
} catch {}

const key = await readFile("./server-key.pem");
const cert = await readFile("./server-cert.pem");

const server = createServer((rawSocket) => {
  const secureSocket = new TLSSocket(rawSocket, {
    key,
    cert,
    isServer: true,
  });

  secureSocket.on("secureConnect", () => {
    console.log("Secure connection established");
    secureSocket.write("hello from server");
    secureSocket.end();
  });

  secureSocket.on("data", (data) => {
    console.log("Received from client:", data.toString());
  });
});

server.listen(SOCKET_PATH, () => {
  console.log(`Server listening on ${SOCKET_PATH}`);
});
