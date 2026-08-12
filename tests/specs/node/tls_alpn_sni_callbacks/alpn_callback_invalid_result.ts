// Test that ALPNCallback returning a protocol not in the client's list
// emits ERR_TLS_ALPN_CALLBACK_INVALID_RESULT.

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
    ALPNCallback: ({ protocols: _protocols }) => {
      // Return a protocol the client did NOT offer
      return "spdy/3.1";
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
      rejectUnauthorized: false,
      ALPNProtocols: ["h2", "http/1.1"],
    },
    () => {
      // Should not reach here
    },
  );

  client.on("error", () => {
    // Client may error when server resets
  });
});

server.on("tlsClientError", (err) => {
  console.log("tlsClientError code:", err.code);
  server.close();
});
