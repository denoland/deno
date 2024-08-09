// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "./test_util.ts";

const cert = Deno.readTextFileSync("tests/testdata/tls/localhost.crt");
const key = Deno.readTextFileSync("tests/testdata/tls/localhost.key");
const caCerts = [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")];

async function pair(opt?: Deno.QuicTransportOptions): Promise<
  [Deno.QuicConn, Deno.QuicConn, Deno.QuicListener]
> {
  const listener = await Deno.listenQuic({
    hostname: "localhost",
    port: 0,
    cert,
    key,
    alpnProtocols: ["deno-test"],
    ...opt,
  });

  const [server, client] = await Promise.all([
    listener.accept(),
    Deno.connectQuic({
      hostname: "localhost",
      port: listener.addr.port,
      caCerts,
      alpnProtocols: ["deno-test"],
      ...opt,
    }),
  ]);

  assertEquals(server.protocol, "deno-test");
  assertEquals(client.protocol, "deno-test");
  assertEquals(client.remoteAddr, listener.addr);

  return [server, client, listener];
}

Deno.test("bidirectional stream", async () => {
  const [server, client, listener] = await pair();

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

  listener.close({ closeCode: 0, reason: "" });
  client.close({ closeCode: 0, reason: "" });
});

Deno.test("unidirectional stream", async () => {
  const [server, client, listener] = await pair();

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

  listener.close({ closeCode: 0, reason: "" });
  client.close({ closeCode: 0, reason: "" });
});

Deno.test("datagrams", async () => {
  const [server, client, listener] = await pair();

  const encoded = (new TextEncoder()).encode("hi!");

  await server.sendDatagram(encoded);

  const data = await client.readDatagram();
  assertEquals(data, encoded);

  listener.close({ closeCode: 0, reason: "" });
  client.close({ closeCode: 0, reason: "" });
});

Deno.test("closing", async () => {
  const [server, client, listener] = await pair();

  server.close({ closeCode: 42, reason: "hi!" });

  assertEquals(await client.closed, { closeCode: 42, reason: "hi!" });

  listener.close({ closeCode: 0, reason: "" });
});

Deno.test("max concurrent streams", async () => {
  const [server, client, listener] = await pair({
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

  listener.close({ closeCode: 0, reason: "" });
  server.close({ closeCode: 0, reason: "" });
  client.close({ closeCode: 0, reason: "" });
});

Deno.test("incoming", async () => {
  const listener = await Deno.listenQuic({
    hostname: "localhost",
    port: 0,
    cert,
    key,
    alpnProtocols: ["deno-test"],
  });

  const connect = () =>
    Deno.connectQuic({
      hostname: "localhost",
      port: listener.addr.port,
      caCerts,
      alpnProtocols: ["deno-test"],
    });

  const c1p = connect();
  const i1 = await listener.incoming();
  const server = await i1.accept();
  const client = await c1p;

  assertEquals(server.protocol, "deno-test");
  assertEquals(client.protocol, "deno-test");
  assertEquals(client.remoteAddr, listener.addr);

  listener.close({ closeCode: 0, reason: "" });
  client.close({ closeCode: 0, reason: "" });
});
