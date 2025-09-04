// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals } from "@std/assert";
import { execCode } from "../unit/test_util.ts";
import { createSocket, type Socket } from "node:dgram";

const listenPort = 4503;
const listenPort2 = 4504;

Deno.test("[node/dgram] udp ref and unref", {
  permissions: { read: true, run: true, net: true },
}, async () => {
  const { promise, resolve } = Promise.withResolvers<void>();

  const udpSocket = createSocket("udp4");
  udpSocket.bind(listenPort);

  udpSocket.unref();
  udpSocket.ref();

  let data;
  udpSocket.on("message", (buffer, _rinfo) => {
    data = Uint8Array.from(buffer);
    udpSocket.close();
  });
  udpSocket.on("close", () => {
    resolve();
  });

  const conn = await Deno.listenDatagram({
    port: listenPort2,
    transport: "udp",
  });
  await conn.send(new Uint8Array([0, 1, 2, 3]), {
    transport: "udp",
    port: listenPort,
    hostname: "127.0.0.1",
  });

  await promise;
  conn.close();
  assertEquals(data, new Uint8Array([0, 1, 2, 3]));
});

Deno.test("[node/dgram] udp unref", {
  permissions: { read: true, run: true, net: true },
}, async () => {
  const [statusCode, _output] = await execCode(`
      import { createSocket } from "node:dgram";
      const udpSocket = createSocket('udp4');
      udpSocket.bind(${listenPort2});
      // This should let the program to exit without waiting for the
      // udp socket to close.
      udpSocket.unref();
      udpSocket.on('message', (buffer, rinfo) => {
      });
    `);
  assertEquals(statusCode, 0);
});

Deno.test("[node/dgram] createSocket, reuseAddr option", async () => {
  const { promise, resolve } = Promise.withResolvers<string>();
  const socket0 = createSocket({ type: "udp4", reuseAddr: true });
  let socket1: Socket | undefined;
  socket0.bind(0, "0.0.0.0", () => {
    const port = socket0.address().port;
    socket1 = createSocket({ type: "udp4", reuseAddr: true });
    socket1.bind(port, "0.0.0.0", () => {
      const socket = createSocket({ type: "udp4" });
      socket.send("hello", port, "localhost", () => {
        socket.close();
      });
    });
    socket1.on("message", (msg) => {
      resolve(msg.toString());
    });
  });
  socket0.on("message", (msg) => {
    resolve(msg.toString());
  });
  assertEquals(await promise, "hello");
  socket0.close();
  socket1?.close();
});
