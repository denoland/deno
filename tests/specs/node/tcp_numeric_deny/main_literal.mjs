// Verify that literal 127.0.0.1 is denied by --deny-net.
// On Windows, only literal IPs are testable because getaddrinfo does not
// resolve decimal/hex numeric aliases.

import net from "node:net";

const port = 12345;

function tryConnect(host) {
  return new Promise((resolve) => {
    const sock = net.connect({ host, port }, () => {
      sock.destroy();
      resolve("OK");
    });
    sock.on("error", (err) => {
      resolve(err.code || err.message);
    });
  });
}

const literal = await tryConnect("127.0.0.1");

console.log(`literal:${literal}`);
