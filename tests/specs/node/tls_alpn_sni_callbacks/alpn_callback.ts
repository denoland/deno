// Test that ALPNCallback on tls.createServer works correctly.
// The server's ALPNCallback should be invoked with the client's offered
// protocols and the selected protocol should be negotiated.

import tls from "node:tls";
import { readFileSync } from "node:fs";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

let alpnCallbackInvoked = false;

const server = tls.createServer(
  {
    cert,
    key,
    ALPNCallback: ({ servername, protocols }) => {
      alpnCallbackInvoked = true;
      console.log("ALPNCallback called");
      console.log("protocols:", JSON.stringify(protocols));
      // Select h2 if offered
      if (protocols.includes("h2")) {
        return "h2";
      }
      return protocols[0];
    },
  },
  (socket) => {
    console.log("server negotiatedProtocol:", socket.alpnProtocol);
    socket.end();
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
      console.log("client negotiatedProtocol:", client.alpnProtocol);
      console.log("ALPNCallback invoked:", alpnCallbackInvoked);
      client.end();
      server.close();
    },
  );
});
