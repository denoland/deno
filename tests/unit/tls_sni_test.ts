// Copyright 2018-2026 the Deno authors. MIT license.
import { assertEquals, assertRejects } from "./test_util.ts";
const { resolverSymbol, serverNameSymbol, tlsKeyResolverInvalidators } =
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
  Deno[Deno.internal];

const cert = Deno.readTextFileSync("tests/testdata/tls/localhost.crt");
const key = Deno.readTextFileSync("tests/testdata/tls/localhost.key");
const certEcc = Deno.readTextFileSync("tests/testdata/tls/localhost_ecc.crt");
const keyEcc = Deno.readTextFileSync("tests/testdata/tls/localhost_ecc.key");

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
  async function listenResolverAlpn() {
    const resolutions: [string, string[]][] = [];
    const opts: unknown = {
      hostname: "localhost",
      port: 0,
      [resolverSymbol]: (sni: string, info: { alpnProtocols: string[] }) => {
        resolutions.push([sni, info.alpnProtocols]);
        if (info.alpnProtocols.includes("acme-tls/1")) {
          // TLS-ALPN-01 style: per-connection ALPN override, never cached
          return { cert, key, alpnProtocols: ["acme-tls/1"], noCache: true };
        }
        return { cert, key };
      },
    };
    // @ts-ignore Trust me
    const listener = Deno.listenTls(opts);

    async function connect(alpnProtocols?: string[]): Promise<string | null> {
      const conn = await Deno.connectTls({
        hostname: "localhost",
        [serverNameSymbol]: "server-1",
        port: listener.addr.port,
        alpnProtocols,
      } as Deno.ConnectTlsOptions);
      const serverConn = await listener.accept();
      const { 0: info } = await Promise.all([
        conn.handshake(),
        serverConn.handshake(),
      ]);
      conn.close();
      serverConn.close();
      return info.alpnProtocol;
    }

    // Regular handshakes share a single cached resolution.
    assertEquals(await connect(), null);
    assertEquals(await connect(), null);
    // The resolver overrides ALPN per connection and opts out of caching,
    // so every challenge handshake resolves freshly.
    assertEquals(await connect(["acme-tls/1"]), "acme-tls/1");
    assertEquals(await connect(["acme-tls/1"]), "acme-tls/1");

    assertEquals(resolutions, [
      ["server-1", []],
      ["server-1", ["acme-tls/1"]],
      ["server-1", ["acme-tls/1"]],
    ]);
    listener.close();
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  async function listenResolverInvalidate() {
    let calls = 0;
    const resolve = (_sni: string) => {
      calls++;
      return { cert, key };
    };
    const opts: unknown = {
      hostname: "localhost",
      port: 0,
      [resolverSymbol]: resolve,
    };
    // @ts-ignore Trust me
    const listener = Deno.listenTls(opts);

    async function connect() {
      const conn = await Deno.connectTls({
        hostname: "localhost",
        [serverNameSymbol]: "server-1",
        port: listener.addr.port,
      });
      const serverConn = await listener.accept();
      await Promise.all([conn.handshake(), serverConn.handshake()]);
      conn.close();
      serverConn.close();
    }

    await connect();
    await connect();
    assertEquals(calls, 1);

    const invalidate = tlsKeyResolverInvalidators.get(resolve);
    invalidate("server-1");

    await connect();
    assertEquals(calls, 2);
    listener.close();
  },
);
