// Test that SNICallback on tls.createServer works correctly.
// The server's SNICallback should be invoked with the client's servername
// and the selected SecureContext should be used for the connection.

import tls from "node:tls";
import { readFileSync } from "node:fs";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

let sniCallbackInvoked = false;

const server = tls.createServer(
  {
    cert,
    key,
    SNICallback: (servername, cb) => {
      sniCallbackInvoked = true;
      console.log("SNICallback called with:", servername);
      // Return the same context (in a real app, you'd select based on hostname)
      const ctx = tls.createSecureContext({ cert, key });
      cb(null, ctx);
    },
  },
  (socket) => {
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
    },
    () => {
      console.log("client connected");
      console.log("SNICallback invoked:", sniCallbackInvoked);
      client.end();
      server.close();
    },
  );
});
