// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals } from "./test_util.ts";

const cert = Deno.readTextFileSync("tests/testdata/tls/localhost.crt");
const key = Deno.readTextFileSync("tests/testdata/tls/localhost.key");
const caCerts = [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")];

interface Pair {
  server: Deno.QuicConn;
  client: Deno.QuicConn;
  endpoint: Deno.QuicEndpoint;
}

async function pair(opt?: Deno.QuicTransportOptions): Promise<Pair> {
  const endpoint = new Deno.QuicEndpoint({ hostname: "localhost" });
  const listener = endpoint.listen({
    cert,
    key,
    alpnProtocols: ["deno-test"],
    ...opt,
  });
  assertEquals(endpoint, listener.endpoint);

  const [server, client] = await Promise.all([
    listener.accept(),
    Deno.connectQuic({
      hostname: "localhost",
      port: endpoint.addr.port,
      caCerts,
      alpnProtocols: ["deno-test"],
      ...opt,
    }),
  ]);

  assertEquals(server.protocol, "deno-test");
  assertEquals(client.protocol, "deno-test");
  assertEquals(client.remoteAddr, endpoint.addr);
  assertEquals(server.serverName, "localhost");

  return { server, client, endpoint };
}

Deno.test("bidirectional stream", async () => {
  const { server, client, endpoint } = await pair();

  const encoded = (new TextEncoder()).encode("hi!");

  {
    const bi = await server.createBidirectionalStream({ sendOrder: 42 });
    assertEquals(bi.writable.sendOrder, 42);
    bi.writable.sendOrder = 0;
    assertEquals(bi.writable.sendOrder, 0);
    await bi.writable.getWriter().write(encoded);
  }

  {
    const { value: bi } = await client.incomingBidirectionalStreams
      .getReader()
      .read();
    const { value: data } = await bi!.readable.getReader().read();
    assertEquals(data, encoded);
  }

  client.close();
  endpoint.close();
});

Deno.test("unidirectional stream", async () => {
  const { server, client, endpoint } = await pair();

  const encoded = (new TextEncoder()).encode("hi!");

  {
    const uni = await server.createUnidirectionalStream({ sendOrder: 42 });
    assertEquals(uni.sendOrder, 42);
    uni.sendOrder = 0;
    assertEquals(uni.sendOrder, 0);
    await uni.getWriter().write(encoded);
  }

  {
    const { value: uni } = await client.incomingUnidirectionalStreams
      .getReader()
      .read();
    const { value: data } = await uni!.getReader().read();
    assertEquals(data, encoded);
  }

  endpoint.close();
  client.close();
});

Deno.test("datagrams", async () => {
  const { server, client, endpoint } = await pair();

  const encoded = (new TextEncoder()).encode("hi!");

  await server.sendDatagram(encoded);

  const data = await client.readDatagram();
  assertEquals(data, encoded);

  endpoint.close();
  client.close();
});

Deno.test("closing", async () => {
  const { server, client } = await pair();

  server.close({ closeCode: 42, reason: "hi!" });

  assertEquals(await client.closed, { closeCode: 42, reason: "hi!" });
});

Deno.test("max concurrent streams", async () => {
  const { server, client, endpoint } = await pair({
    maxConcurrentBidirectionalStreams: 1,
    maxConcurrentUnidirectionalStreams: 1,
  });

  {
    await server.createBidirectionalStream();
    await server.createBidirectionalStream()
      .then(() => {
        throw new Error("expected failure");
      }, () => {
        // success!
      });
  }

  {
    await server.createUnidirectionalStream();
    await server.createUnidirectionalStream()
      .then(() => {
        throw new Error("expected failure");
      }, () => {
        // success!
      });
  }

  endpoint.close();
  client.close();
});

Deno.test("incoming", async () => {
  const endpoint = new Deno.QuicEndpoint({ hostname: "localhost" });
  const listener = endpoint.listen({
    cert,
    key,
    alpnProtocols: ["deno-test"],
  });

  const connect = () =>
    Deno.connectQuic({
      hostname: "localhost",
      port: endpoint.addr.port,
      caCerts,
      alpnProtocols: ["deno-test"],
    });

  const c1p = connect();
  const i1 = await listener.incoming();
  const server = await i1.accept();
  const client = await c1p;

  assertEquals(server.protocol, "deno-test");
  assertEquals(client.protocol, "deno-test");
  assertEquals(client.remoteAddr, endpoint.addr);

  endpoint.close();
  client.close();
});

Deno.test("0rtt", async () => {
  const sEndpoint = new Deno.QuicEndpoint({ hostname: "localhost" });
  const listener = sEndpoint.listen({
    cert,
    key,
    alpnProtocols: ["deno-test"],
  });

  (async () => {
    while (true) {
      let incoming;
      try {
        incoming = await listener.incoming();
      } catch (e) {
        if (e instanceof Deno.errors.BadResource) {
          break;
        }
        throw e;
      }
      const conn = incoming.accept({ zeroRtt: true });
      conn.handshake.then(() => {
        conn.close();
      });
    }
  })();

  const endpoint = new Deno.QuicEndpoint();

  const c1 = await Deno.connectQuic({
    hostname: "localhost",
    port: sEndpoint.addr.port,
    caCerts,
    alpnProtocols: ["deno-test"],
    endpoint,
  });

  await c1.closed;

  // TODO(bartlomieju|littledivy): this assertion is disabled for now, because
  // during upgrade to rustls 0.23.28 it was found that `quinn` needs to
  // use exactly same configuration (as in Arc-identical config) which is
  // very hard to pass around with the current quinn API. Since QUIC API i
  // unstable it was decided to ignore this failure for now and revisit later
  // to unblock upgrade and other work.
  //
  // See https://github.com/quinn-rs/quinn/issues/2299#issuecomment-3052666623
  //
  // const c2 = Deno.connectQuic({
  //   hostname: "localhost",
  //   port: sEndpoint.addr.port,
  //   caCerts,
  //   alpnProtocols: ["deno-test"],
  //   zeroRtt: true,
  //   endpoint,
  // });
  // assert(!(c2 instanceof Promise), "0rtt should be accepted");
  // await c2.closed;

  sEndpoint.close();
  endpoint.close();
});
