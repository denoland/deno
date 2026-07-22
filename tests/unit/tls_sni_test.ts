// Copyright 2018-2026 the Deno authors. MIT license.
import { assertEquals, assertRejects } from "./test_util.ts";
// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const { resolverSymbol, serverNameSymbol } = Deno[Deno.internal];

const cert = Deno.readTextFileSync("tests/testdata/tls/localhost.crt");
const key = Deno.readTextFileSync("tests/testdata/tls/localhost.key");
const caCert = Deno.readTextFileSync("tests/testdata/tls/RootCA.pem");
const certEcc = Deno.readTextFileSync("tests/testdata/tls/localhost_ecc.crt");
const keyEcc = Deno.readTextFileSync("tests/testdata/tls/localhost_ecc.key");

async function resolvesWithin(promise: Promise<unknown>, ms: number) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      promise.then(() => "resolved"),
      new Promise<"blocked">((resolve) => {
        timer = setTimeout(() => resolve("blocked"), ms);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

Deno.test(
  { permissions: { net: true, read: true } },
  async function listenResolver() {
    const sniRequests: string[] = [];
    const keys: Record<string, { cert: string; key: string }> = {
      "server-1": { cert, key },
      "server-2": { cert: certEcc, key: keyEcc },
      "fail-server-3": { cert: "(invalid)", key: "(bad)" },
    };
    const opts: unknown = {
      hostname: "localhost",
      port: 0,
      [resolverSymbol]: (sni: string) => {
        sniRequests.push(sni);
        return keys[sni]!;
      },
    };
    // @ts-ignore Trust me
    const listener = Deno.listenTls(opts);

    for (
      const server of ["server-1", "server-2", "fail-server-3", "fail-server-4"]
    ) {
      const conn = await Deno.connectTls({
        hostname: "localhost",
        [serverNameSymbol]: server,
        port: listener.addr.port,
      });
      const serverConn = await listener.accept();
      if (server.startsWith("fail-")) {
        await assertRejects(async () => await conn.handshake());
        await assertRejects(async () => await serverConn.handshake());
      } else {
        await conn.handshake();
        await serverConn.handshake();
      }
      conn.close();
      serverConn.close();
    }

    assertEquals(sniRequests, [
      "server-1",
      "server-2",
      "fail-server-3",
      "fail-server-4",
    ]);
    listener.close();
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  async function listenResolverDoesNotBlockUnrelatedSni() {
    const slowStarted = Promise.withResolvers<void>();
    const releaseSlow = Promise.withResolvers<void>();
    const handshakes: Promise<unknown>[] = [];
    const connections: { close(): void }[] = [];

    const opts: unknown = {
      hostname: "localhost",
      port: 0,
      [resolverSymbol]: async (sni: string) => {
        if (sni === "slow-server") {
          slowStarted.resolve();
          await releaseSlow.promise;
        }
        return { cert, key };
      },
    };
    // @ts-ignore Trust me
    const listener = Deno.listenTls(opts);

    try {
      const slowClient = await Deno.connectTls({
        hostname: "localhost",
        [serverNameSymbol]: "slow-server",
        port: listener.addr.port,
        caCerts: [caCert],
        unsafelyDisableHostnameVerification: true,
      });
      const slowServer = await listener.accept();
      connections.push(slowClient, slowServer);
      handshakes.push(slowClient.handshake(), slowServer.handshake());

      await slowStarted.promise;

      const fastClient = await Deno.connectTls({
        hostname: "localhost",
        [serverNameSymbol]: "fast-server",
        port: listener.addr.port,
        caCerts: [caCert],
        unsafelyDisableHostnameVerification: true,
      });
      const fastServer = await listener.accept();
      connections.push(fastClient, fastServer);
      const fastHandshake = Promise.all([
        fastClient.handshake(),
        fastServer.handshake(),
      ]);
      handshakes.push(fastHandshake);

      assertEquals(
        await resolvesWithin(fastHandshake, 1_000),
        "resolved",
      );

      releaseSlow.resolve();
      await Promise.all(handshakes);
    } finally {
      releaseSlow.resolve();
      listener.close();
      for (const conn of connections) {
        try {
          conn.close();
        } catch {
          // The connection may already have been closed by the handshake path.
        }
      }
      await Promise.allSettled(handshakes);
    }
  },
);
