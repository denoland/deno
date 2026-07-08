// SNICallback enables the Acceptor-based handshake, so the ALPN check
// happens in finishAccept (Accepted::into_connection) rather than in JS.
// With no overlap between the server's ALPNProtocols and the client's,
// into_connection fails before any TLS connection exists: the server
// flushes a no_application_protocol alert to the client and destroys the
// socket while that encrypted write is still in flight.
//
// Regression test for a use-after-free: TLSWrapInner::teardown() must mark
// the wrap dead (alive=false) even when tls_conn was never created, so the
// completing write callback does not dereference the freed TLSWrapInner.

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
    ALPNProtocols: ["h2"],
    SNICallback: (_servername, cb) => {
      cb(null, tls.createSecureContext({ cert, key }));
    },
  },
  (_socket) => {
    console.log("ERROR: secureConnection should not fire");
  },
);

server.on("tlsClientError", (err) => {
  console.log("tlsClientError:", (err as { code?: string }).code);
  server.close();
});

server.listen(0, () => {
  const { port } = server.address() as { port: number };

  const client = tls.connect(
    {
      port,
      host: "localhost",
      servername: "localhost",
      rejectUnauthorized: false,
      ALPNProtocols: ["http/1.1"],
    },
    () => {
      console.log("ERROR: client should not connect");
    },
  );

  client.on("error", (err) => {
    console.log("client error:", (err as { code?: string }).code);
  });
});
