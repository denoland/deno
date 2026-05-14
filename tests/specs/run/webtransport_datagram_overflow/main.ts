// Copyright 2018-2026 the Deno authors. All rights reserved. MIT license.

// Regression test: sending multiple datagrams that exceed the incoming
// high-water mark (default 1) must not cause an infinite loop.

import { decodeBase64 } from "@std/encoding/base64";

const cert = Deno.readTextFileSync("../../../testdata/tls/localhost.crt");
const key = Deno.readTextFileSync("../../../testdata/tls/localhost.key");
const certHash = await crypto.subtle.digest(
  "SHA-256",
  decodeBase64(cert.split("\n").slice(1, -2).join("")),
);

Deno.test("datagram overflow does not hang", async () => {
  const server = new Deno.QuicEndpoint({
    hostname: "localhost",
    port: 0,
  });
  const listener = server.listen({
    cert,
    key,
    alpnProtocols: ["h3"],
  });

  const serverReady = Promise.withResolvers<void>();

  (async () => {
    for await (const incoming of listener) {
      const conn = await incoming.accept();
      const wt = await Deno.upgradeWebTransport(conn);
      await wt.ready;
      serverReady.resolve();
      // Intentionally do NOT drain wt.datagrams.readable,
      // so incoming datagrams queue up and trigger the overflow path.
    }
  })();

  const client = new WebTransport(
    `https://localhost:${server.addr.port}/path`,
    {
      serverCertificateHashes: [{
        algorithm: "sha-256",
        value: certHash,
      }],
    },
  );
  await client.ready;
  await serverReady.promise;

  const writer = client.datagrams.writable.getWriter();

  // Send two datagrams to exceed the default incomingHighWaterMark of 1.
  // Before the fix, the second datagram would cause an infinite loop on
  // the server side.
  await writer.write(new Uint8Array([1]));
  await writer.write(new Uint8Array([2]));

  // Give the server time to receive and process both datagrams.
  // If the overflow loop is infinite, this timer will never fire.
  await new Promise((resolve) => setTimeout(resolve, 500));

  client.close();
  server.close();
});
