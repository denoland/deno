// Test that both ALPNCallback and SNICallback work together.

import tls from "node:tls";
import { readFileSync } from "node:fs";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

const server = tls.createServer(
  {
    cert,
    key,
    SNICallback: (servername, cb) => {
      console.log("SNICallback:", servername);
      const ctx = tls.createSecureContext({ cert, key });
      cb(null, ctx);
    },
    ALPNCallback: ({ servername, protocols }) => {
      console.log("ALPNCallback:", servername, JSON.stringify(protocols));
      return "h2";
    },
  },
  (socket) => {
    console.log("server alpn:", socket.alpnProtocol);
    console.log("server servername:", socket.servername);
    socket.end();
  },
);

server.listen(0, () => {
  const { port } = server.address() as { port: number };

  const client = tls.connect(
    {
      port,
      host: "localhost",
      servername: "localhost",
      rejectUnauthorized: false,
      ALPNProtocols: ["h2", "http/1.1"],
    },
    () => {
      console.log("client alpn:", client.alpnProtocol);
      client.end();
      server.close();
    },
  );
});
