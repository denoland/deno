// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects } from "./test_util.ts";
// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const { resolverSymbol, serverNameSymbol } = Deno[Deno.internal];

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
