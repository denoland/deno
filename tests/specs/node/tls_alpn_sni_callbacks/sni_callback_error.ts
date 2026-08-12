// Test that SNICallback passing an error destroys the socket.

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
    SNICallback: (_servername, cb) => {
      cb(new Error("SNI lookup failed"));
    },
  },
  (_socket) => {
    // Should not reach here
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
    },
    () => {
      // Should not reach here
    },
  );

  client.on("error", () => {
    // Client may error when server destroys
  });
});

server.on("tlsClientError", (err) => {
  console.log("tlsClientError:", err.message);
  server.close();
});
