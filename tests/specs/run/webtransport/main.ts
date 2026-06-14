// Copyright 2018-2026 the Deno authors. All rights reserved. MIT license.

import { decodeBase64 } from "@std/encoding/base64";
import { assertEquals } from "@std/assert";

const cert = Deno.readTextFileSync("../../../testdata/tls/localhost.crt");
const certHash = await crypto.subtle.digest(
  "SHA-256",
  decodeBase64(cert.split("\n").slice(1, -2).join("")),
);

Deno.test("WebTransport", async () => {
  const server = new Deno.QuicEndpoint({
    hostname: "localhost",
    port: 0,
  });
  const listener = server.listen({
    cert,
    key: Deno.readTextFileSync("../../../testdata/tls/localhost.key"),
    alpnProtocols: ["h3"],
  });

  (async () => {
    for await (const incoming of listener) {
      const conn = await incoming.accept();
      const wt = await Deno.upgradeWebTransport(conn);

      assertEquals(wt.url, `https://localhost:${server.addr.port}/path`);

      wt.ready.then(() => {
        (async () => {
          for await (const bidi of wt.incomingBidirectionalStreams) {
            bidi.readable.pipeTo(bidi.writable).catch(() => {});
          }
        })();

        (async () => {
          for await (const stream of wt.incomingUnidirectionalStreams) {
            const out = await wt.createUnidirectionalStream();
            stream.pipeTo(out).catch(() => {});
          }
        })();

        wt.datagrams.readable.pipeTo(wt.datagrams.writable);
      });
    }
  })();

  // Connecting with a literal IP host must work for both IPv4 and IPv6. A
  // bracketed IPv6 URL host (e.g. https://[::1]/) needs its brackets stripped
  // before being handed to the QUIC connector. Each check uses its own endpoint
  // bound to the matching address family, since "localhost" only resolves to
  // one of them.
  async function checkConnect(hostname: string) {
    const endpoint = new Deno.QuicEndpoint({ hostname, port: 0 });
    const listener = endpoint.listen({
      cert,
      key: Deno.readTextFileSync("../../../testdata/tls/localhost.key"),
      alpnProtocols: ["h3"],
    });
    (async () => {
      for await (const incoming of listener) {
        const conn = await incoming.accept();
        await Deno.upgradeWebTransport(conn);
      }
    })();

    const host = hostname.includes(":") ? `[${hostname}]` : hostname;
    const client = new WebTransport(
      `https://${host}:${endpoint.addr.port}/path`,
      {
        serverCertificateHashes: [{
          algorithm: "sha-256",
          value: certHash,
        }],
      },
    );
    await client.ready;
    client.close();
    endpoint.close();
  }

  await checkConnect("127.0.0.1");
  await checkConnect("::1");

  const client = new WebTransport(
    `https://localhost:${server.addr.port}/path`,
    {
      serverCertificateHashes: [{
        algorithm: "sha-256",
        value: certHash,
      }],
    },
  );

  await client.ready.then(async () => {
    const bi = await client.createBidirectionalStream();

    {
      const writer = bi.writable.getWriter();
      await writer.write(new Uint8Array([1, 0, 1, 0]));
      writer.releaseLock();
    }

    {
      const reader = bi.readable.getReader();
      assertEquals(await reader.read(), {
        value: new Uint8Array([1, 0, 1, 0]),
        done: false,
      });
      reader.releaseLock();
    }

    {
      const uni = await client.createUnidirectionalStream();
      const writer = uni.getWriter();
      await writer.write(new Uint8Array([0, 2, 0, 2]));
      writer.releaseLock();
    }

    {
      const uni =
        (await client.incomingUnidirectionalStreams.getReader().read()).value;
      const reader = uni!.getReader();
      assertEquals(await reader.read(), {
        value: new Uint8Array([0, 2, 0, 2]),
        done: false,
      });
      reader.releaseLock();
    }

    await client.datagrams.writable.getWriter().write(
      new Uint8Array([3, 0, 3, 0]),
    );
    assertEquals(await client.datagrams.readable.getReader().read(), {
      value: new Uint8Array([3, 0, 3, 0]),
      done: false,
    });

    client.close();
    server.close();
  });
});
