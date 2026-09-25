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

  let abortPrefixReceived: () => void;
  const abortPrefix = new Promise<void>((resolve) =>
    abortPrefixReceived = resolve
  );
  let abortErrorReceived: (error: unknown) => void;
  const abortError = new Promise<unknown>((resolve) =>
    abortErrorReceived = resolve
  );

  (async () => {
    let bidiIndex = 0;
    const serverRetained = [];
    for await (const incoming of listener) {
      const conn = await incoming.accept();
      const wt = await Deno.upgradeWebTransport(conn);

      assertEquals(wt.url, `https://localhost:${server.addr.port}/path`);

      wt.ready.then(() => {
        (async () => {
          for await (const bidi of wt.incomingBidirectionalStreams) {
            serverRetained.push(bidi);
            if (bidiIndex++ === 121) {
              const reader = bidi.readable.getReader();
              const prefix = await reader.read();
              assertEquals(prefix.value, new Uint8Array([0x11, 0x22, 0x33]));
              abortPrefixReceived();
              const pendingRead = reader.read();
              try {
                const result = await pendingRead;
                throw new Error("unexpected observer result: " + result.done);
              } catch (error) {
                abortErrorReceived(error);
              }
              reader.releaseLock();
            } else {
              bidi.readable.pipeTo(bidi.writable).catch((error) => {
                throw error;
              });
            }
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
      await writer.close();
    }

    {
      const reader = bi.readable.getReader();
      assertEquals(await reader.read(), {
        value: new Uint8Array([1, 0, 1, 0]),
        done: false,
      });
      assertEquals(await reader.read(), { value: undefined, done: true });
      reader.releaseLock();
    }

    const retained = [];
    for (let i = 0; i < 120; i++) {
      const cycle = await client.createBidirectionalStream();
      retained.push(cycle);
      const payload = new Uint8Array([i & 0xff, 0xa5, 0x5a, i >> 8]);
      const writer = cycle.writable.getWriter();
      await writer.write(payload);
      await writer.close();
      const reader = cycle.readable.getReader();
      assertEquals(await reader.read(), { value: payload, done: false });
      assertEquals(await reader.read(), { value: undefined, done: true });
      reader.releaseLock();
    }

    {
      const aborted = await client.createBidirectionalStream();
      retained.push(aborted);
      const writer = aborted.writable.getWriter();
      await writer.write(new Uint8Array([0x11, 0x22, 0x33]));
      await abortPrefix;
      await writer.abort(
        new WebTransportError("reset", {
          source: "stream",
          streamErrorCode: 7,
        }),
      );
      const error = await abortError;
      assertEquals(
        (error as Error).message,
        "stream reset by peer: error 91141958510818",
      );
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

Deno.test("WebTransport ignores overridden URL toString", async () => {
  const server = new Deno.QuicEndpoint({
    hostname: "localhost",
    port: 0,
  });
  const listener = server.listen({
    cert,
    key: Deno.readTextFileSync("../../../testdata/tls/localhost.key"),
    alpnProtocols: ["h3"],
  });
  const expectedUrl = `https://localhost:${server.addr.port}/path`;

  const serverDone = (async () => {
    for await (const incoming of listener) {
      const conn = await incoming.accept();
      const wt = await Deno.upgradeWebTransport(conn);
      await wt.ready;
      assertEquals(wt.url, expectedUrl);
      return;
    }
  })();

  const originalToString = URL.prototype.toString;
  URL.prototype.toString = () => "not a url";
  try {
    const client = new WebTransport(expectedUrl, {
      serverCertificateHashes: [{
        algorithm: "sha-256",
        value: certHash,
      }],
    });
    await client.ready;
    client.close();
    await serverDone;
  } finally {
    URL.prototype.toString = originalToString;
    server.close();
  }
});
